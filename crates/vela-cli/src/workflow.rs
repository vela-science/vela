//! The producer loop: next → start → submit.
//!
//! `start` creates one private Attempt against an exact target. `submit`
//! registers an authenticated `vela.submission.v1`, a Vela-issued
//! Registration Record, and one pending Proposal through repository authority.
//! It cannot create Verification, a Decision, an Event, or accepted state.
//! Historical Receipt-era objects remain readable only through compatibility
//! fixtures; this module contains no current Receipt writer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_authority::CedarEvaluationInput;
use vela_authority::runtime_authentication::{
    AuthenticationRequest, RuntimeSessionState, SignedAgentEventSession,
    SignedAgentSubmissionSession, SignedVerificationRecordSession,
};
use vela_protocol::authority::PrincipalSnapshotV1;
use vela_protocol::principal_capability::PrincipalClass;
use vela_protocol::repo;
use vela_protocol::submission_v1::{
    ProducerCheck, RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft,
    SubmissionProvenance, SubmissionV1,
};

pub(crate) fn acquire_canonical_write_barrier(
    frontier: &Path,
    journal_dir: &Path,
) -> Result<crate::frontier_txn::CanonicalWriteBarrier, crate::frontier_txn::FrontierTxnError> {
    #[cfg(test)]
    {
        crate::frontier_txn::FrontierTxn::acquire_write_barrier_for_test(frontier, journal_dir)
    }
    #[cfg(not(test))]
    {
        crate::frontier_txn::FrontierTxn::acquire_write_barrier(frontier, journal_dir)
    }
}

fn lease_args(
    frontier: &Path,
    target: &str,
    actor: &str,
    ttl_seconds: u64,
    prior_claim_event_id: Option<&str>,
    release_reason: Option<&str>,
) -> Value {
    json!({
        "frontier_path": frontier.display().to_string(),
        "obligation_id": target,
        "agent_actor": actor,
        "ttl_seconds": ttl_seconds,
        "prior_claim_event_id": prior_claim_event_id,
        "release_reason": release_reason,
    })
}

fn clone_project(
    project: &vela_protocol::project::Project,
) -> Result<vela_protocol::project::Project, String> {
    serde_json::from_value(serde_json::to_value(project).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

/// Install one already validated and signed event candidate through the same
/// recoverable frontier-wide transaction used by Proposal and Decision writes.
/// The supplied barrier must have been acquired before the Project snapshot
/// was loaded, so the recorded `state_root_before` is the producer's exact
/// scientific base rather than a best-effort observation.
#[derive(Clone, Copy)]
struct EventTransactionBinding {
    operation_namespace: &'static str,
    request_schema: &'static str,
    request_event_id_field: &'static str,
    result_event_id_field: &'static str,
    result_timestamp_field: &'static str,
    publication_summary: &'static str,
    preserve_existing_event_bytes: bool,
}

fn preserve_existing_event_bytes(
    frontier: &Path,
    original: &vela_protocol::project::Project,
    candidate: &vela_protocol::project::Project,
    managed: &mut vela_protocol::repo::ManagedFileSet,
    operation_namespace: &str,
) -> Result<(), String> {
    for original_event in &original.events {
        let candidate_event = candidate
            .events
            .iter()
            .find(|event| event.id == original_event.id)
            .ok_or_else(|| {
                format!(
                    "{operation_namespace} candidate removed existing event {}",
                    original_event.id
                )
            })?;
        if serde_json::to_value(original_event).map_err(|error| error.to_string())?
            != serde_json::to_value(candidate_event).map_err(|error| error.to_string())?
        {
            return Err(format!(
                "{operation_namespace} candidate changed existing event {}",
                original_event.id
            ));
        }
        let relative = format!(".vela/events/{}.json", original_event.id);
        let bytes = std::fs::read(frontier.join(&relative)).map_err(|error| {
            format!(
                "{operation_namespace} cannot preserve existing event bytes at {relative}: {error}"
            )
        })?;
        managed.writes.insert(relative, bytes);
    }
    Ok(())
}

fn transact_event_candidate_with_barrier<F>(
    frontier: &Path,
    barrier: crate::frontier_txn::CanonicalWriteBarrier,
    original: &vela_protocol::project::Project,
    candidate: &vela_protocol::project::Project,
    mut result: Value,
    binding: EventTransactionBinding,
    before_commit: F,
) -> Result<Value, String>
where
    F: FnOnce() -> Result<(), String>,
{
    use crate::config::git_publish::{
        PublicationOutcome, PublicationState, PublishOptions, exact_publication_resume_preflight,
        publication_disabled_reason, publish_exact_delta,
    };
    use crate::frontier_txn::{
        ContentDigest, DeltaDraft, FrontierBinding, FrontierTxn, FrontierTxnPlan,
        FrontierTxnPlanSpec, OperationId, OperationKind, PlannedWrite,
    };

    if result.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(format!(
            "{} candidate is not success-shaped",
            binding.operation_namespace
        ));
    }
    let expected_event_log_root = ContentDigest::parse(format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&original.events)
    ))
    .map_err(|error| error.to_string())?;
    let resulting_event_log_root = ContentDigest::parse(format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&candidate.events)
    ))
    .map_err(|error| error.to_string())?;
    if result.get("state_root_before").and_then(Value::as_str)
        != Some(expected_event_log_root.as_str())
        || result.get("state_root_after").and_then(Value::as_str)
            != Some(resulting_event_log_root.as_str())
    {
        return Err(format!(
            "{} response does not bind its exact transaction roots",
            binding.operation_namespace
        ));
    }
    let event_id = result
        .get(binding.result_event_id_field)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "{} candidate did not return its event identity",
                binding.operation_namespace
            )
        })?;
    let event_id = event_id.to_string();
    if !candidate.events.iter().any(|event| event.id == event_id) {
        return Err(format!(
            "{} candidate does not contain its claimed event",
            binding.operation_namespace
        ));
    }

    let mut request = json!({
        "schema": binding.request_schema,
        "state_root_before": expected_event_log_root.as_str(),
        "state_root_after": resulting_event_log_root.as_str(),
    });
    request[binding.request_event_id_field] = json!(event_id);
    let request_bytes = vela_protocol::canonical::to_canonical_bytes(&request)?;
    let request_root = ContentDigest::hash(&request_bytes);
    let operation_id = OperationId::derive(
        binding.operation_namespace,
        request_root.as_str().as_bytes(),
    );
    let mut managed = repo::render_vela_repo_files(frontier, candidate)?;
    if binding.preserve_existing_event_bytes {
        preserve_existing_event_bytes(
            frontier,
            original,
            candidate,
            &mut managed,
            binding.operation_namespace,
        )?;
    }
    let writes = PlannedWrite::from_managed_files(managed).map_err(|error| error.to_string())?;
    let draft = DeltaDraft::prepare(frontier, writes).map_err(|error| error.to_string())?;
    let layout = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.frontier-layout.internal.v1",
        "frontier_id": original.frontier_id(),
        "paths": draft
            .delta
            .writes()
            .iter()
            .map(|write| write.path.as_str())
            .collect::<Vec<_>>(),
    }))?;
    let mut resulting_event_ids = candidate
        .events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    resulting_event_ids.sort();
    let plan = FrontierTxnPlan::new(
        FrontierTxnPlanSpec {
            kind: OperationKind::Maintenance,
            operation_id,
            request_root,
            frontier: FrontierBinding::new(frontier, original.frontier_id(), &layout)
                .map_err(|error| error.to_string())?,
            fixed_time: result
                .get(binding.result_timestamp_field)
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "{} candidate did not return its timestamp",
                        binding.operation_namespace
                    )
                })?
                .to_string(),
            expected_event_log_root,
            resulting_event_log_root,
            resulting_event_ids,
            // The event root is the authoritative event-log CAS. The canonical
            // delta separately binds every rendered Project preimage.
            read_set: Vec::new(),
            result: result.clone(),
        },
        draft.delta.clone(),
    )
    .map_err(|error| error.to_string())?;
    let mut transaction = FrontierTxn::prepare_with_barrier(barrier, plan, draft)
        .map_err(|error| error.to_string())?;
    let public = transaction
        .resolved_public_writes()
        .map_err(|error| error.to_string())?;
    let delta_root = transaction
        .plan()
        .canonical_delta
        .root()
        .as_str()
        .to_string();
    before_commit()?;
    transaction
        .mark_committed()
        .map_err(|error| error.to_string())?;
    transaction.install().map_err(|error| error.to_string())?;
    transaction.complete().map_err(|error| error.to_string())?;
    let publish_opts = PublishOptions::new(false);
    let publication_disabled = publication_disabled_reason(frontier, &publish_opts);
    let publication_delta = if publication_disabled.is_some() {
        None
    } else {
        publication_delta(frontier, &delta_root, public)?
    };
    let publication = match publication_delta.as_ref() {
        Some(delta) => match exact_publication_resume_preflight(frontier, delta, &publish_opts) {
            Ok(preflight) => publish_exact_delta(
                frontier,
                binding.publication_summary,
                std::slice::from_ref(&event_id),
                delta,
                preflight,
                &publish_opts,
            )
            .unwrap_or_else(|error| PublicationOutcome {
                state: PublicationState::Unknown {
                    reason: error.to_string(),
                },
                recovery_command: None,
            }),
            Err(outcome) => outcome,
        },
        None => PublicationOutcome {
            state: PublicationState::Uncommitted {
                candidate: None,
                reason: publication_disabled
                    .unwrap_or_else(|| "lease transaction had no public Git delta".to_string()),
            },
            recovery_command: None,
        },
    };
    result
        .as_object_mut()
        .ok_or_else(|| "lease candidate result is not an object".to_string())?
        .insert(
            "publication".to_string(),
            serde_json::to_value(publication).map_err(|error| error.to_string())?,
        );
    Ok(result)
}

fn transact_lease_candidate_with_barrier<F>(
    frontier: &Path,
    barrier: crate::frontier_txn::CanonicalWriteBarrier,
    original: &vela_protocol::project::Project,
    candidate: &vela_protocol::project::Project,
    result: Value,
    before_commit: F,
) -> Result<Value, String>
where
    F: FnOnce() -> Result<(), String>,
{
    transact_event_candidate_with_barrier(
        frontier,
        barrier,
        original,
        candidate,
        result,
        EventTransactionBinding {
            operation_namespace: "lease",
            request_schema: "vela.lease-event-request.internal.v1",
            request_event_id_field: "claim_event_id",
            result_event_id_field: "claim_event_id",
            result_timestamp_field: "claimed_at",
            publication_summary: "work",
            preserve_existing_event_bytes: true,
        },
        before_commit,
    )
}

fn active_repository_signing_key(
    authority: &crate::cli::LoadedRepositoryAuthority,
) -> Result<(String, String), String> {
    let sequence = u64::try_from(authority.verification.authority_record_count + 1)
        .map_err(|_| "repository-authority sequence exceeds u64".to_string())?;
    if authority.history.authority_keyset.threshold != 1 {
        return Err(
            "routine local repository-authority writes currently require a one-key threshold"
                .into(),
        );
    }
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
    let [key] = active.as_slice() else {
        return Err(format!(
            "routine local repository-authority writes require exactly one active key at sequence {sequence}; found {}",
            active.len()
        ));
    };
    Ok((key.key_id.clone(), key.public_key.clone()))
}

fn transact_repository_authority_lease(
    frontier: &Path,
    barrier: crate::frontier_txn::CanonicalWriteBarrier,
    authority: crate::cli::LoadedRepositoryAuthority,
    signed_candidate_event: &vela_protocol::events::StateEvent,
    mut claim: Value,
) -> Result<Value, String> {
    let claimant_pubkey = claim
        .get("claimant_pubkey")
        .and_then(Value::as_str)
        .ok_or_else(|| "lease claim did not return its claimant key".to_string())?;
    let mut authentication =
        SignedAgentEventSession::from_event(signed_candidate_event, claimant_pubkey)?;
    let intent_digest = crate::frontier_txn::ContentDigest::hash(
        vela_protocol::canonical::to_canonical_bytes(signed_candidate_event)?,
    )
    .as_str()
    .to_string();
    let recorded_at = signed_candidate_event.timestamp.clone();
    let authorization_input = CedarEvaluationInput {
        schema: authority.policy_material.schema.clone(),
        policies: authority.policy_material.policies.clone(),
        entities: authority.policy_material.entities.clone(),
        principal: format!(
            "Agent::{}",
            serde_json::to_string(&signed_candidate_event.actor.id)
                .expect("serializing an actor ID cannot fail")
        ),
        principal_class: PrincipalClass::Agent,
        action: "work_claim".into(),
        resource: format!(
            "Frontier::{}",
            serde_json::to_string(&authority.history.frontier_id)
                .expect("serializing a frontier ID cannot fail")
        ),
        context: json!({"exact": true}),
    };
    let event_draft = crate::authority_transaction::AuthorityEventDraft {
        kind: signed_candidate_event.kind.clone(),
        target: signed_candidate_event.target.clone(),
        actor: signed_candidate_event.actor.clone(),
        timestamp: signed_candidate_event.timestamp.clone(),
        reason: signed_candidate_event.reason.clone(),
        before_hash: signed_candidate_event.before_hash.clone(),
        after_hash: signed_candidate_event.after_hash.clone(),
        payload: signed_candidate_event.payload.clone(),
        caveats: signed_candidate_event.caveats.clone(),
    };
    let (key_id, public_key) = active_repository_signing_key(&authority)?;
    let mut signer =
        crate::repository_authority_provider::SshAgentRepositoryAuthoritySigner::from_environment(
            key_id,
            &public_key,
        )?;
    let executable =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let binary_sha256 = crate::authority_transaction::execution_binary_sha256(&executable)?;
    let mut transaction = crate::authority_transaction::prepare_authority_transaction(
        barrier,
        frontier,
        crate::authority_transaction::AuthorityTransactionRequest {
            history: authority.history,
            intent_digest,
            principal: PrincipalSnapshotV1 {
                principal_id: signed_candidate_event.actor.id.clone(),
                principal_class: PrincipalClass::Agent,
                display_name: None,
                affiliation: None,
                account_links: vec![signed_candidate_event.actor.id.clone()],
            },
            authentication_request: AuthenticationRequest {
                principal_id: signed_candidate_event.actor.id.clone(),
                principal_class: PrincipalClass::Agent,
                transaction_at: recorded_at.clone(),
            },
            runtime_session_state: RuntimeSessionState::default(),
            authorization_input,
            delegation: None,
            semantic_approvals: Vec::new(),
            event_drafts: vec![event_draft],
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
    let public = transaction
        .resolved_public_writes()
        .map_err(|error| error.to_string())?;
    let delta_root = transaction.canonical_delta_root().to_string();
    let result = transaction.result.clone();
    transaction
        .mark_committed()
        .map_err(|error| error.to_string())?;
    transaction.install().map_err(|error| error.to_string())?;
    transaction.complete().map_err(|error| error.to_string())?;
    let [event_id] = result.event_ids.as_slice() else {
        return Err(
            "repository-authority lease transaction did not produce exactly one event".to_string(),
        );
    };
    let object = claim
        .as_object_mut()
        .ok_or_else(|| "lease claim result is not an object".to_string())?;
    object.insert("claim_event_id".into(), json!(event_id));
    object.insert(
        "state_root_before".into(),
        json!(result.before_event_log_root),
    );
    object.insert(
        "state_root_after".into(),
        json!(result.after_event_log_root),
    );
    object.insert(
        "authority_record_id".into(),
        json!(result.authority_record_id),
    );
    object.insert(
        "publication".into(),
        serde_json::to_value({
            use crate::config::git_publish::{
                PublicationOutcome, PublicationState, PublishOptions,
                exact_publication_resume_preflight, publication_disabled_reason,
                publish_exact_delta,
            };
            let publish_opts = PublishOptions::new(false);
            let disabled = publication_disabled_reason(frontier, &publish_opts);
            let delta = if disabled.is_some() {
                None
            } else {
                publication_delta(frontier, &delta_root, public)?
            };
            match delta.as_ref() {
                Some(delta) => {
                    match exact_publication_resume_preflight(frontier, delta, &publish_opts) {
                        Ok(preflight) => publish_exact_delta(
                            frontier,
                            "work",
                            std::slice::from_ref(event_id),
                            delta,
                            preflight,
                            &publish_opts,
                        )
                        .unwrap_or_else(|error| PublicationOutcome {
                            state: PublicationState::Unknown {
                                reason: error.to_string(),
                            },
                            recovery_command: None,
                        }),
                        Err(outcome) => outcome,
                    }
                }
                None => PublicationOutcome {
                    state: PublicationState::Uncommitted {
                        candidate: None,
                        reason: disabled.unwrap_or_else(|| {
                            "repository-authority lease transaction had no public Git delta"
                                .to_string()
                        }),
                    },
                    recovery_command: None,
                },
            }
        })
        .map_err(|error| error.to_string())?,
    );
    Ok(claim)
}

pub(crate) fn transact_proposal_withdrawal<F>(
    frontier: &Path,
    proposal_id: &str,
    actor_id: &str,
    reason: &str,
    load_key: F,
) -> Result<Value, String>
where
    F: FnOnce() -> Result<ed25519_dalek::SigningKey, String>,
{
    if reason.trim().is_empty() {
        return Err("withdrawal reason must not be empty".to_string());
    }
    let journal_dir = frontier_transaction_journal_dir(frontier)?;
    let barrier = acquire_canonical_write_barrier(frontier, &journal_dir)
        .map_err(|error| error.to_string())?;
    let original = repo::load_from_path(frontier)?;
    if let Some(event) =
        vela_protocol::proposals::existing_proposal_withdrawal(frontier, &original, proposal_id)?
    {
        if event.actor.id != actor_id {
            return Err("withdrawn proposal belongs to a different producer".to_string());
        }
        return Ok(json!({
            "ok": true,
            "command": "proposal.withdraw",
            "proposal_id": proposal_id,
            "withdrawal_event_id": event.id,
            "withdrawn_at": event.timestamp,
            "idempotent": true,
            "key_read": false,
            "state_root_before": format!("sha256:{}", vela_protocol::events::event_log_hash(&original.events)),
            "state_root_after": format!("sha256:{}", vela_protocol::events::event_log_hash(&original.events)),
        }));
    }
    let proposal = original
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("proposal {proposal_id} does not exist"))?;
    let authorization =
        vela_protocol::proposals::proposal_withdrawal_authorization(frontier, proposal)?;
    if proposal.actor.id != actor_id || authorization.identity_binding.actor_id != actor_id {
        return Err("withdrawal actor is not the producer bound to this Proposal".to_string());
    }
    let key = load_key()?;
    let withdrawn_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let encoded = serde_json::to_value(&original).map_err(|error| error.to_string())?;
    let mut candidate: vela_protocol::project::Project =
        serde_json::from_value(encoded).map_err(|error| error.to_string())?;
    let event = vela_protocol::proposals::apply_proposal_withdrawal(
        frontier,
        &mut candidate,
        proposal_id,
        actor_id,
        reason,
        &withdrawn_at,
        &key,
    )?;
    let state_root_before = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&original.events)
    );
    let state_root_after = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&candidate.events)
    );
    let result = json!({
        "ok": true,
        "command": "proposal.withdraw",
        "proposal_id": proposal_id,
        "withdrawal_event_id": event.id,
        "withdrawn_at": withdrawn_at,
        "idempotent": false,
        "key_read": true,
        "state_root_before": state_root_before,
        "state_root_after": state_root_after,
    });
    transact_event_candidate_with_barrier(
        frontier,
        barrier,
        &original,
        &candidate,
        result,
        EventTransactionBinding {
            operation_namespace: "proposal-withdrawal",
            request_schema: "vela.proposal-withdrawal-request.internal.v1",
            request_event_id_field: "withdrawal_event_id",
            result_event_id_field: "withdrawal_event_id",
            result_timestamp_field: "withdrawn_at",
            publication_summary: "proposal withdraw",
            preserve_existing_event_bytes: true,
        },
        || Ok(()),
    )
}

/// The pre-loaded briefing for a target — the compounding payload the
/// session starts from. Problem-shaped targets get the full task packet;
/// rich campaign targets also carry their non-authorizing coordination task.
#[derive(Debug)]
struct PreparedWorkBriefing {
    value: Value,
    target_task_binding: Option<vela_edge::target_index::TargetTaskBindingV1>,
}

fn briefing_from_project(
    frontier: &Path,
    target: &str,
    project: &vela_protocol::project::Project,
    trust_anchor: Option<&vela_edge::frontier_repository::RepositoryTrustAnchor>,
    authority_events: &[vela_protocol::authority::AuthorityEventV1],
) -> Result<PreparedWorkBriefing, String> {
    let head = vela_protocol::events::event_log_hash(&project.events);
    let finding_target = project.findings.iter().any(|finding| finding.id == target);
    let indexed = if finding_target {
        None
    } else {
        vela_edge::frontier_next::target_index_selection_for_target_with_trust_anchor_and_authority(
            project,
            frontier,
            target,
            trust_anchor,
            authority_events,
        )?
    };
    let packet = indexed.as_ref().map_or_else(
        || crate::server::tools::briefing_for_target(project, frontier, target),
        |selection| selection.packet.clone(),
    );
    let target_task_binding = indexed.as_ref().map(|selection| selection.binding.clone());
    let task = if finding_target {
        None
    } else if let Some(selection) = indexed {
        Some(selection.task)
    } else {
        vela_edge::frontier_next::campaign_task_for_target(project, frontier, target)?
    };
    let mut offer = json!({
        "schema": "vela.next_offer.v0.1",
        "target": target,
        "pinned_state": {
            "frontier_id": project.frontier_id().to_string(),
            "event_log_hash": head,
        },
        "briefing": packet,
    });
    if let Some(task) = task {
        offer["task"] = task;
    }
    Ok(PreparedWorkBriefing {
        value: offer,
        target_task_binding,
    })
}

/// The session directory for a target within a frontier.
pub(crate) fn session_dir(frontier: &Path, target: &str) -> PathBuf {
    let mut safe: String = target
        .chars()
        .take(48)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    while safe.contains("--") {
        safe = safe.replace("--", "-");
    }
    let safe = safe.trim_matches('-');
    let safe = if safe.is_empty() { "target" } else { safe };
    let target_root = hex::encode(Sha256::digest(target.as_bytes()));
    frontier
        .join(".vela")
        .join("work")
        .join(format!("{safe}--{target_root}"))
}

const ATTEMPT_SCHEMA: &str = "vela.attempt.v1";
const TASK_CONTRACT_SCHEMA: &str = "vela.task-contract.internal.v1";
const ATTEMPT_MAX_BYTES: usize = 2 * 1024 * 1024;

/// One private, ignored producer session. It is coordination and authoring
/// context, never canonical scientific state or an authority object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Attempt {
    pub schema: String,
    pub attempt_id: String,
    pub target: String,
    pub frontier_id: String,
    pub base_event_log_root: String,
    pub base_nonlease_event_log_root: String,
    /// Era-1 non-lease commitment present when this private session opened.
    ///
    /// Legacy sessions omit it. Repository-authority sessions bind it into
    /// their session identity so unrelated leases may coexist while any
    /// scientific or authority change still invalidates submission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_authority_nonlease_event_log_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_git_commit_oid: Option<String>,
    pub source_git_state: String,
    pub actor: String,
    pub created_at: String,
    pub lease: WorkSessionLease,
    pub task_contract: TaskContract,
    pub task_contract_root: String,
    pub submission_builder: SubmissionBuilderAttemptFacts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_task_binding: Option<vela_edge::target_index::TargetTaskBindingV1>,
    pub briefing: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkSessionLease {
    pub claim_event_id: String,
    pub claimant_pubkey: String,
    pub claimed_at: String,
    pub lease_ttl_seconds: u64,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TaskContract {
    pub schema: String,
    pub objective: String,
    pub completion_condition: String,
    pub allowed_actions: Vec<String>,
    pub forbidden_actions: Vec<String>,
    pub required_outputs: Vec<String>,
    pub required_checks: Vec<String>,
    pub escalation_path: String,
    pub authority_ceiling: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SubmissionBuilderAttemptFacts {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verifier_results: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedAttempt {
    pub record: Attempt,
    pub relative_dir: String,
}

fn task_contract(briefing: &Value, target: &str) -> TaskContract {
    let body = briefing.get("briefing").unwrap_or(briefing);
    let objective = body
        .get("statement")
        .and_then(Value::as_str)
        .map(|statement| format!("Produce decision-relevant evidence for: {statement}"))
        .or_else(|| {
            body.get("objective")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| format!("Produce decision-relevant evidence for target {target}."));
    let mut required_outputs = body
        .get("allowed_outputs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|output| output.get("type").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if required_outputs.is_empty() {
        required_outputs.push(
            "one evidence artifact or one deliberately informative negative result".to_string(),
        );
    }
    required_outputs.sort();
    required_outputs.dedup();
    TaskContract {
        schema: TASK_CONTRACT_SCHEMA.to_string(),
        objective,
        completion_condition:
            "Submit one valid Submission whose evidence and caveats address this target."
                .to_string(),
        allowed_actions: vec![
            "inspect the pinned frontier and task briefing".to_string(),
            "run frozen verifiers and private search or experiment loops".to_string(),
            "create evidence artifacts and submit one bounded Submission".to_string(),
            "deposit an informative failed or partial attempt".to_string(),
        ],
        forbidden_actions: vec![
            "accept, reject, apply, finalize, or sign a truth-bearing proposal".to_string(),
            "read or use a human signing key".to_string(),
            "hand-edit accepted events or derived frontier views".to_string(),
            "treat producer or model output as a verifier or authority verdict".to_string(),
        ],
        required_outputs,
        required_checks: vec![
            "run every producer-side check claimed by the Submission and report its actual outcome"
                .to_string(),
            "state at least one caveat; if no material limitation is known, say so explicitly"
                .to_string(),
            "keep artifacts frontier-relative, bounded, and content-addressed at submission"
                .to_string(),
        ],
        escalation_path:
            "Submit for review; an authorized principal may later accept or reject the exact Proposal."
                .to_string(),
        authority_ceiling:
            "Producer evidence only. The Attempt can create a Submission and Proposal; it cannot create acceptance."
                .to_string(),
    }
}

fn sha256_root(value: &impl Serialize) -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(value)?
    ))
}

fn attempt_id(
    frontier_id: &str,
    target: &str,
    actor: &str,
    claim_event_id: &str,
    task_contract_root: &str,
    target_task_binding_root: Option<&str>,
    authority_nonlease_event_log_root: Option<&str>,
) -> Result<String, String> {
    let mut preimage = json!({
        "schema": ATTEMPT_SCHEMA,
        "frontier_id": frontier_id,
        "target": target,
        "actor": actor,
        "claim_event_id": claim_event_id,
        "task_contract_root": task_contract_root,
    });
    if let Some(binding_root) = target_task_binding_root {
        preimage["target_task_binding_root"] = json!(binding_root);
    }
    if let Some(authority_root) = authority_nonlease_event_log_root {
        preimage["authority_nonlease_event_log_root"] = json!(authority_root);
    }
    Ok(format!(
        "vat_{}",
        vela_protocol::canonical::sha256_canonical(&preimage)?
    ))
}

fn nonlease_event_log_root(events: &[vela_protocol::events::StateEvent]) -> String {
    format!(
        "sha256:{}",
        vela_protocol::events::nonlease_event_log_hash(events)
    )
}

fn repository_authority_nonlease_event_log_root(
    project: &vela_protocol::project::Project,
    authority: &crate::cli::LoadedRepositoryAuthority,
) -> Result<String, String> {
    let legacy_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    );
    let events = authority
        .history
        .authority_events
        .iter()
        .filter(|event| event.content.kind != vela_protocol::events::EventKind::AttemptClaimed)
        .collect::<Vec<_>>();
    vela_protocol::authority_history::authority_event_log_root(&legacy_root, &events)
}

fn repository_submission_materialization_candidate(
    frontier: &Path,
    proposal: vela_protocol::proposals::StateProposal,
) -> Result<vela_protocol::project::Project, String> {
    // Reload from canonical repository bytes rather than accepting the
    // effective workflow Project. The latter may contain detached
    // repository-authority lease overlays used only to validate the active
    // Attempt; those overlays must never enter ordinary derived views.
    let mut candidate = repo::load_from_path(frontier)?;
    vela_protocol::proposals::insert_pending_in_frontier(&mut candidate, proposal)?;
    Ok(candidate)
}

fn apply_repository_authority_leases(
    project: &mut vela_protocol::project::Project,
    authority: &crate::cli::LoadedRepositoryAuthority,
) -> Result<(), String> {
    for event in authority.ordered_events()? {
        if event.content.kind != vela_protocol::events::EventKind::AttemptClaimed {
            continue;
        }
        let reducer_event = vela_protocol::events::StateEvent {
            schema: vela_protocol::events::EVENT_SCHEMA.into(),
            id: event.id.clone(),
            kind: event.content.kind.clone(),
            target: event.content.target.clone(),
            actor: event.content.actor.clone(),
            timestamp: event.content.timestamp.clone(),
            reason: event.content.reason.clone(),
            before_hash: event.content.before_hash.clone(),
            after_hash: event.content.after_hash.clone(),
            payload: event.content.payload.clone(),
            caveats: event.content.caveats.clone(),
            signature: None,
        };
        vela_protocol::reducer::apply_event(project, &reducer_event)?;
    }
    vela_protocol::project::recompute_stats(project);
    Ok(())
}

fn load_project_with_repository_authority(
    frontier: &Path,
) -> Result<
    (
        vela_protocol::project::Project,
        Option<crate::cli::LoadedRepositoryAuthority>,
    ),
    String,
> {
    let mut project = repo::load_from_path(frontier)?;
    let authority = crate::cli::load_repository_authority(frontier, &project)?;
    if let Some(loaded) = &authority {
        apply_repository_authority_leases(&mut project, loaded)?;
    }
    Ok((project, authority))
}

fn source_git_commit(frontier: &Path) -> (Option<String>, String) {
    match crate::git_hardened::output(frontier, &["rev-parse", "--verify", "HEAD^{commit}"]) {
        Ok(output) if output.status.success() => {
            let oid = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if (40..=64).contains(&oid.len())
                && oid
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                (Some(oid), "pinned".to_string())
            } else {
                (None, "unavailable_invalid_git_oid".to_string())
            }
        }
        Ok(_) => (None, "unavailable_not_a_git_commit".to_string()),
        Err(_) => (None, "unavailable_git_not_installed".to_string()),
    }
}

fn encoded_attempt(session: &Attempt) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(session)
        .map_err(|error| format!("encode Attempt record: {error}"))?;
    bytes.push(b'\n');
    if bytes.len() > ATTEMPT_MAX_BYTES {
        return Err(format!(
            "Attempt record is {} bytes; limit is {ATTEMPT_MAX_BYTES} bytes",
            bytes.len()
        ));
    }
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
fn preflight_work_session_size(
    target: &str,
    frontier_id: &str,
    base_event_log_root: &str,
    base_nonlease_event_log_root: &str,
    base_authority_nonlease_event_log_root: Option<String>,
    source_git_commit_oid: Option<String>,
    source_git_state: &str,
    actor: &str,
    ttl_seconds: u64,
    task_contract: TaskContract,
    task_contract_root: String,
    target_task_binding: Option<vela_edge::target_index::TargetTaskBindingV1>,
    briefing: Value,
) -> Result<(), String> {
    // These placeholders are at least as long as the generated identity and
    // timestamp fields. This rejects an impossible session before key lookup
    // or candidate signing; the exact record is measured again before commit.
    let timestamp_placeholder = "0".repeat(64);
    let session = Attempt {
        schema: ATTEMPT_SCHEMA.to_string(),
        attempt_id: format!("vat_{}", "0".repeat(64)),
        target: target.to_string(),
        frontier_id: frontier_id.to_string(),
        base_event_log_root: base_event_log_root.to_string(),
        base_nonlease_event_log_root: base_nonlease_event_log_root.to_string(),
        base_authority_nonlease_event_log_root,
        source_git_commit_oid,
        source_git_state: source_git_state.to_string(),
        actor: actor.to_string(),
        created_at: timestamp_placeholder.clone(),
        lease: WorkSessionLease {
            claim_event_id: format!("vev_{}", "0".repeat(64)),
            claimant_pubkey: "0".repeat(64),
            claimed_at: timestamp_placeholder.clone(),
            lease_ttl_seconds: ttl_seconds,
            expires_at: timestamp_placeholder,
        },
        task_contract,
        task_contract_root,
        submission_builder: SubmissionBuilderAttemptFacts::default(),
        target_task_binding,
        briefing,
    };
    encoded_attempt(&session).map(|_| ())
}

fn write_attempt(frontier: &Path, session: &Attempt) -> Result<PathBuf, String> {
    use std::io::Write;

    let bytes = encoded_attempt(session)?;
    let vela = frontier.join(".vela");
    let metadata = std::fs::symlink_metadata(&vela).map_err(|error| {
        format!(
            "inspect frontier private directory {}: {error}",
            vela.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "frontier private directory must be a real directory: {}",
            vela.display()
        ));
    }
    let work = vela.join("work");
    match std::fs::symlink_metadata(&work) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!(
                "Attempt root must be a real directory: {}",
                work.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&work)
                .map_err(|error| format!("create Attempt root {}: {error}", work.display()))?;
        }
        Err(error) => {
            return Err(format!("inspect Attempt root {}: {error}", work.display()));
        }
    }
    let directory = session_dir(frontier, &session.target);
    match std::fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!(
                "Attempt must be a real directory: {}",
                directory.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&directory)
                .map_err(|error| format!("create Attempt {}: {error}", directory.display()))?;
        }
        Err(error) => {
            return Err(format!("inspect Attempt {}: {error}", directory.display()));
        }
    }
    let path = directory.join("attempt.json");
    if let Ok(metadata) = std::fs::symlink_metadata(&path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(format!(
            "Attempt record must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    let temporary = directory.join(format!(
        ".attempt-{}-{}.tmp",
        std::process::id(),
        &session.attempt_id[4..20]
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options
            .open(&temporary)
            .map_err(|error| format!("create Attempt record: {error}"))?;
        file.write_all(&bytes)
            .map_err(|error| format!("write Attempt record: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("persist Attempt record: {error}"))?;
        std::fs::rename(&temporary, &path)
            .map_err(|error| format!("install Attempt record: {error}"))?;
        Ok(path.clone())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn parse_attempt(path: &Path) -> Result<Attempt, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("inspect Attempt {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "Attempt record must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    if metadata.len() > ATTEMPT_MAX_BYTES as u64 {
        return Err(format!("Attempt record is too large: {}", path.display()));
    }
    let bytes =
        std::fs::read(path).map_err(|error| format!("read Attempt {}: {error}", path.display()))?;
    let session: Attempt = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "parse Attempt {}: {error}; remove this private stale Attempt and rerun `vela start <target> --as <actor>`",
            path.display()
        )
    })?;
    validate_attempt_record(&session).map_err(|error| format!("{error}: {}", path.display()))?;
    Ok(session)
}

fn validate_attempt_record(session: &Attempt) -> Result<(), String> {
    if session.schema != ATTEMPT_SCHEMA {
        return Err(format!("unsupported Attempt schema {}", session.schema));
    }
    if session.task_contract.schema != TASK_CONTRACT_SCHEMA
        || sha256_root(&session.task_contract)? != session.task_contract_root
    {
        return Err("Attempt task contract does not match its content root".to_string());
    }
    let expected_attempt_id = attempt_id(
        &session.frontier_id,
        &session.target,
        &session.actor,
        &session.lease.claim_event_id,
        &session.task_contract_root,
        session
            .target_task_binding
            .as_ref()
            .map(|binding| binding.binding_root.as_str()),
        session.base_authority_nonlease_event_log_root.as_deref(),
    )?;
    if session.attempt_id != expected_attempt_id {
        return Err("Attempt identity does not match its closed root preimage".to_string());
    }
    if let Some(binding) = &session.target_task_binding {
        binding.validate().map_err(|error| {
            format!("Attempt target binding does not match its closed content root: {error}")
        })?;
        if binding.frontier_id != session.frontier_id || binding.target_id != session.target {
            return Err(
                "Attempt target binding does not match its Frontier and target".to_string(),
            );
        }
        if binding.claim_read_set.event_log_root != session.base_event_log_root
            || session.source_git_commit_oid.as_deref()
                != Some(binding.claim_read_set.git_commit.as_str())
            || session.source_git_state != "pinned"
        {
            return Err("Attempt target binding does not match its claim read set".to_string());
        }
    }
    Ok(())
}

fn validate_active_session(frontier: &Path, actor: &str, session: &Attempt) -> Result<(), String> {
    if session.actor != actor {
        return Err(format!(
            "Attempt {} belongs to {}, not {actor}",
            session.target, session.actor
        ));
    }
    let (project, authority) = load_project_with_repository_authority(frontier)?;
    if session.frontier_id != project.frontier_id() {
        return Err(format!(
            "Attempt {} belongs to a different Frontier",
            session.target
        ));
    }
    match (
        session.base_authority_nonlease_event_log_root.as_deref(),
        authority.as_ref(),
    ) {
        (Some(expected), Some(authority)) => {
            let actual = repository_authority_nonlease_event_log_root(&project, authority)?;
            if actual != expected {
                return Err(format!(
                    "Attempt {} has repository-authority changes from its pinned state",
                    session.target
                ));
            }
        }
        (Some(_), None) => {
            return Err(
                "repository-authority Attempt lost its migrated authority history".to_string(),
            );
        }
        (None, Some(_)) => {
            return Err(
                "legacy Attempt cannot cross the repository-authority migration boundary; remove it and rerun `vela start`"
                    .to_string(),
            );
        }
        (None, None) => {}
    }
    revalidate_work_session_target_binding(frontier, &project, session)?;
    let current = project
        .attempt_claims
        .iter()
        .find(|claim| claim.obligation_id == session.target)
        .ok_or_else(|| format!("work target {} has no frontier lease", session.target))?;
    if current.claimant_actor != actor
        || current.claimant_pubkey != session.lease.claimant_pubkey
        || current.claim_event_id.as_deref() != Some(session.lease.claim_event_id.as_str())
    {
        return Err(format!(
            "Attempt {} no longer owns the exact Frontier lease",
            session.target
        ));
    }
    let expires =
        vela_protocol::events::attempt_lease_expiry(&current.claimed_at, current.lease_ttl_seconds)
            .map_err(|error| format!("work lease: {error}"))?;
    if expires <= chrono::Utc::now() {
        return Err(format!("Attempt {} lease has expired", session.target));
    }
    Ok(())
}

fn revalidate_work_session_target_binding(
    frontier: &Path,
    project: &vela_protocol::project::Project,
    session: &Attempt,
) -> Result<(), String> {
    let Some(binding) = &session.target_task_binding else {
        return Ok(());
    };
    let loaded_anchor =
        crate::target_index::load_user_repository_trust_anchor(&project.frontier_id())?;
    let repository_anchor = loaded_anchor
        .as_ref()
        .map(|loaded| crate::target_index::boundary_anchor(&loaded.anchor));
    let authority_events = crate::target_index::load_verified_authority_events(frontier, project)?;
    vela_edge::target_index::revalidate_target_task_binding_with_authority_events(
        project,
        frontier,
        binding,
        repository_anchor.as_ref(),
        &authority_events,
    )
}

/// Return the exact causal root for an Attempt whose scientific base is still
/// current.
///
/// An Attempt deliberately pins `base_event_log_root` before its own
/// coordination event, so the task describes the scientific state the
/// producer actually started from. Attempts separately pin the non-lease event
/// set, so unrelated leases may
/// coexist while every scientific, provenance, or authority-event change
/// fails closed. The non-lease root is required because Attempts are
/// private scratch and have no compatibility lane.
#[cfg(test)]
fn attempt_causal_event_root(
    project: &vela_protocol::project::Project,
    session: &Attempt,
) -> Result<String, String> {
    let matching_claims = project
        .events
        .iter()
        .filter(|event| event.id == session.lease.claim_event_id)
        .collect::<Vec<_>>();
    if matching_claims.len() != 1 {
        return Err("Attempt must resolve to exactly one Frontier lease event".to_string());
    }
    let claim = matching_claims[0];
    if claim.kind != vela_protocol::events::EVENT_KIND_ATTEMPT_CLAIMED
        || claim.actor.id != session.actor
        || claim.payload.get("obligation_id").and_then(Value::as_str)
            != Some(session.target.as_str())
        || claim.payload.get("claimant_pubkey").and_then(Value::as_str)
            != Some(session.lease.claimant_pubkey.as_str())
    {
        return Err("Attempt lease event does not match its signed facts".to_string());
    }
    if nonlease_event_log_root(&project.events) != session.base_nonlease_event_log_root {
        return Err(format!(
            "Attempt Frontier has non-lease changes from its pinned state; remove the private Attempt and rerun `vela start {} --as {}`",
            session.target, session.actor
        ));
    }
    Ok(format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    ))
}

/// Resolve an explicit Attempt ID or infer the one active Attempt owned by this
/// actor. Other actors' Attempts never create ambiguity.
pub(crate) fn resolve_attempt(
    frontier: &Path,
    actor: &str,
    requested_attempt: Option<&str>,
) -> Result<ResolvedAttempt, String> {
    let root = frontier.join(".vela").join("work");
    let entries = std::fs::read_dir(&root)
        .map_err(|_| "no active Attempt; run `vela start <target>` first".to_string())?;
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("enumerate Attempts: {error}"))?;
        let path = entry.path().join("attempt.json");
        if !path.exists() {
            continue;
        }
        let record = parse_attempt(&path)?;
        if record.actor != actor {
            continue;
        }
        if validate_active_session(frontier, actor, &record).is_err() {
            continue;
        }
        let relative_dir = entry
            .path()
            .strip_prefix(frontier)
            .map_err(|_| "Attempt escaped the Frontier".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        candidates.push(ResolvedAttempt {
            record,
            relative_dir,
        });
    }
    candidates.sort_by(|left, right| left.record.target.cmp(&right.record.target));
    if let Some(attempt_id) = requested_attempt {
        let mut exact = candidates
            .into_iter()
            .filter(|candidate| candidate.record.attempt_id == attempt_id)
            .collect::<Vec<_>>();
        return match exact.len() {
            1 => Ok(exact.remove(0)),
            0 => Err(format!(
                "no active Attempt {attempt_id} belongs to {actor}; run `vela start <target> --as {actor}`"
            )),
            _ => Err(format!(
                "Attempt identity {attempt_id} is ambiguous; run `vela check . --strict`"
            )),
        };
    }
    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err(format!(
            "no active Attempt for {actor}; run `vela start <target> --as {actor}` first"
        )),
        count => Err(format!(
            "{actor} has {count} active Attempts ({}); select one with `vela submit --attempt <attempt-id>`",
            candidates
                .iter()
                .map(|attempt| attempt.record.attempt_id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Claim/refresh a lease and install the single typed ignored session record.
/// CLI and MCP both call this function.
enum WorkWriteBarrier {
    Legacy(crate::frontier_txn::CanonicalWriteBarrier),
    Repository(crate::frontier_txn::CanonicalWriteBarrier),
}

fn acquire_work_write_barrier(
    frontier: &Path,
    journal_dir: &Path,
) -> Result<(bool, WorkWriteBarrier), String> {
    let preliminary = repo::load_from_path(frontier)?;
    let repository_authority_enabled =
        crate::cli::load_repository_authority(frontier, &preliminary)?.is_some();
    let barrier = if repository_authority_enabled {
        WorkWriteBarrier::Repository(
            crate::frontier_txn::FrontierTxn::acquire_repository_authority_write_barrier(
                frontier,
                journal_dir,
            )
            .map_err(|error| error.to_string())?,
        )
    } else {
        WorkWriteBarrier::Legacy(
            acquire_canonical_write_barrier(frontier, journal_dir)
                .map_err(|error| error.to_string())?,
        )
    };
    Ok((repository_authority_enabled, barrier))
}

pub(crate) fn open_session(
    frontier: &Path,
    target: &str,
    actor: &str,
    ttl_seconds: u64,
) -> Result<Value, String> {
    open_session_with_after_barrier(frontier, target, actor, ttl_seconds, || Ok(()))
}

fn open_session_with_after_barrier<F>(
    frontier: &Path,
    target: &str,
    actor: &str,
    ttl_seconds: u64,
    after_barrier: F,
) -> Result<Value, String>
where
    F: FnOnce() -> Result<(), String>,
{
    if ttl_seconds == 0 || ttl_seconds > vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS {
        return Err(format!(
            "work lease TTL must be between 1 and {} seconds",
            vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS
        ));
    }
    let journal_dir = frontier_transaction_journal_dir(frontier)?;
    let (repository_authority_enabled, barrier) =
        acquire_work_write_barrier(frontier, &journal_dir)?;
    after_barrier()?;
    // Pin the producer's scientific base before the coordination lease adds
    // its own event. The claim remains the exact live-lease identity, while
    // the session and optional campaign task describe what state the producer
    // actually started from.
    let (base_project, repository_authority) = load_project_with_repository_authority(frontier)?;
    if repository_authority_enabled != repository_authority.is_some() {
        return Err("repository authority changed while acquiring the work barrier".into());
    }
    if let Some(current) = base_project
        .attempt_claims
        .iter()
        .find(|claim| claim.obligation_id == target)
        .filter(|claim| claim.claimant_actor == actor && claim.lease_ttl_seconds > 0)
    {
        let expires = vela_protocol::events::attempt_lease_expiry(
            &current.claimed_at,
            current.lease_ttl_seconds,
        )
        .map_err(|error| format!("work lease: {error}"))?;
        if expires > chrono::Utc::now() {
            let path = session_dir(frontier, target).join("attempt.json");
            let session = parse_attempt(&path).map_err(|error| {
                format!(
                    "work target {target} already has an active lease for {actor}, but its private session is unavailable: {error}; wait for the in-flight `vela start` command or release the lease with `vela start {target} --drop --as {actor}`"
                )
            })?;
            validate_active_session(frontier, actor, &session)?;
            return Ok(json!({
                "ok": true,
                "idempotent": true,
                "target": target,
                "claim": {
                    "ok": true,
                    "idempotent": true,
                    "claim_event_id": &session.lease.claim_event_id,
                    "claimant_pubkey": &session.lease.claimant_pubkey,
                    "claimed_at": &session.lease.claimed_at,
                },
                "briefing": &session.briefing,
                "attempt": session,
                "attempt_path": path.display().to_string(),
            }));
        }
    }
    let loaded_anchor =
        crate::target_index::load_user_repository_trust_anchor(&base_project.frontier_id())?;
    let repository_anchor = loaded_anchor
        .as_ref()
        .map(|loaded| crate::target_index::boundary_anchor(&loaded.anchor));
    let authority_events =
        crate::target_index::load_verified_authority_events(frontier, &base_project)?;
    let prepared = briefing_from_project(
        frontier,
        target,
        &base_project,
        repository_anchor.as_ref(),
        &authority_events,
    )?;
    let briefing = prepared.value;
    let target_task_binding = prepared.target_task_binding;
    let base_event_log_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&base_project.events)
    );
    let base_nonlease_event_log_root = nonlease_event_log_root(&base_project.events);
    let base_authority_nonlease_event_log_root = repository_authority
        .as_ref()
        .map(|authority| repository_authority_nonlease_event_log_root(&base_project, authority))
        .transpose()?;
    let (source_git_commit_oid, source_git_state) = source_git_commit(frontier);
    let contract = task_contract(&briefing, target);
    let task_contract_root = sha256_root(&contract)?;
    preflight_work_session_size(
        target,
        &base_project.frontier_id(),
        &base_event_log_root,
        &base_nonlease_event_log_root,
        base_authority_nonlease_event_log_root.clone(),
        source_git_commit_oid.clone(),
        &source_git_state,
        actor,
        ttl_seconds,
        contract.clone(),
        task_contract_root.clone(),
        target_task_binding.clone(),
        briefing.clone(),
    )?;
    let args = lease_args(frontier, target, actor, ttl_seconds, None, None);
    let mut candidate = clone_project(&base_project)?;
    // Historical proof-state records predate the explicit non-lease root.
    // Bind it only while the old full root still proves that every non-lease
    // event is unchanged, then append the operational coordination event.
    vela_protocol::proposals::backfill_nonlease_proof_root(&mut candidate);
    let claim = vela_edge::vela_agent_mcp::apply_claim_task_to_project(&args, &mut candidate)?;
    if claim.get("ok").and_then(Value::as_bool) != Some(true) {
        let owner = claim
            .get("already_claimed_by")
            .and_then(Value::as_str)
            .unwrap_or("another actor");
        return Err(format!("work target {target} is already leased by {owner}"));
    }
    if candidate.events.len() != base_project.events.len() + 1 {
        return Err("work claim candidate did not append exactly one signed event".into());
    }
    let signed_candidate_event = candidate
        .events
        .last()
        .cloned()
        .ok_or_else(|| "work claim candidate has no signed event".to_string())?;
    let claim = match (barrier, repository_authority) {
        (WorkWriteBarrier::Legacy(barrier), None) => transact_lease_candidate_with_barrier(
            frontier,
            barrier,
            &base_project,
            &candidate,
            claim,
            || Ok(()),
        )?,
        (WorkWriteBarrier::Repository(barrier), Some(authority)) => {
            transact_repository_authority_lease(
                frontier,
                barrier,
                authority,
                &signed_candidate_event,
                claim,
            )?
        }
        _ => return Err("work writer authority changed during planning".into()),
    };
    let claim_event_id = claim
        .get("claim_event_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "lease claim did not return its event identity".to_string())?
        .to_string();
    let claimant_pubkey = claim
        .get("claimant_pubkey")
        .and_then(Value::as_str)
        .ok_or_else(|| "lease claim did not return its claimant key".to_string())?
        .to_string();
    let claimed_at = claim
        .get("claimed_at")
        .and_then(Value::as_str)
        .ok_or_else(|| "lease claim did not return its timestamp".to_string())?
        .to_string();
    let expires_at =
        vela_protocol::events::attempt_lease_expiry(&claimed_at, ttl_seconds)?.to_rfc3339();
    let attempt_id = attempt_id(
        &base_project.frontier_id(),
        target,
        actor,
        &claim_event_id,
        &task_contract_root,
        target_task_binding
            .as_ref()
            .map(|binding| binding.binding_root.as_str()),
        base_authority_nonlease_event_log_root.as_deref(),
    )?;
    let session = Attempt {
        schema: ATTEMPT_SCHEMA.to_string(),
        attempt_id,
        target: target.to_string(),
        frontier_id: base_project.frontier_id().to_string(),
        base_event_log_root,
        base_nonlease_event_log_root,
        base_authority_nonlease_event_log_root,
        source_git_commit_oid,
        source_git_state,
        actor: actor.to_string(),
        created_at: claimed_at.clone(),
        lease: WorkSessionLease {
            claim_event_id,
            claimant_pubkey,
            claimed_at,
            lease_ttl_seconds: ttl_seconds,
            expires_at,
        },
        task_contract: contract,
        task_contract_root,
        submission_builder: SubmissionBuilderAttemptFacts::default(),
        target_task_binding,
        briefing: briefing.clone(),
    };
    // The conservative preflight happened before either writer crossed its
    // marker. Recheck the exact private record before installing it.
    validate_attempt_record(&session)?;
    encoded_attempt(&session)?;
    let path = write_attempt(frontier, &session)?;
    Ok(json!({
        "ok": true,
        "idempotent": false,
        "target": target,
        "claim": claim,
        "briefing": briefing,
        "attempt": session,
        "attempt_path": path.display().to_string(),
    }))
}

/// Release the exact current lease with a signed zero-TTL update, then remove
/// producer scratch. Failure before the event is saved preserves the session.
pub(crate) fn release_session(
    frontier: &Path,
    target: &str,
    actor: &str,
    reason: &str,
) -> Result<Value, String> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("work --drop requires a non-empty release reason".to_string());
    }
    let journal_dir = frontier_transaction_journal_dir(frontier)?;
    let (repository_authority_enabled, barrier) =
        acquire_work_write_barrier(frontier, &journal_dir)?;
    let (project, repository_authority) = load_project_with_repository_authority(frontier)?;
    if repository_authority_enabled != repository_authority.is_some() {
        return Err("repository authority changed while acquiring the work barrier".into());
    }
    let lease = project
        .attempt_claims
        .iter()
        .find(|claim| claim.obligation_id == target)
        .ok_or_else(|| format!("work target {target} has no frontier lease"))?;
    if lease.claimant_actor != actor {
        return Err(format!(
            "work target {target} is leased by {}, not {actor}",
            lease.claimant_actor
        ));
    }
    let expires_at =
        vela_protocol::events::attempt_lease_expiry(&lease.claimed_at, lease.lease_ttl_seconds)
            .map_err(|error| format!("work lease: {error}"))?;
    if lease.lease_ttl_seconds == 0 || expires_at <= chrono::Utc::now() {
        return Err(format!("work target {target} has no current live lease"));
    }
    let prior = lease
        .claim_event_id
        .as_deref()
        .ok_or_else(|| format!("work target {target} lease identity is unavailable"))?
        .to_string();
    let args = lease_args(frontier, target, actor, 0, Some(&prior), Some(reason));
    let mut candidate = clone_project(&project)?;
    let release = vela_edge::vela_agent_mcp::apply_claim_task_to_project(&args, &mut candidate)?;
    if release.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(format!("work target {target} lease was not released"));
    }
    if candidate.events.len() != project.events.len() + 1 {
        return Err("work release candidate did not append exactly one signed event".into());
    }
    let signed_candidate_event = candidate
        .events
        .last()
        .cloned()
        .ok_or_else(|| "work release candidate has no signed event".to_string())?;
    let release = match (barrier, repository_authority) {
        (WorkWriteBarrier::Legacy(barrier), None) => transact_lease_candidate_with_barrier(
            frontier,
            barrier,
            &project,
            &candidate,
            release,
            || Ok(()),
        )?,
        (WorkWriteBarrier::Repository(barrier), Some(authority)) => {
            transact_repository_authority_lease(
                frontier,
                barrier,
                authority,
                &signed_candidate_event,
                release,
            )?
        }
        _ => return Err("work writer authority changed during release planning".into()),
    };
    let directory = session_dir(frontier, target);
    let session_dir_removed = match std::fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => false,
        Ok(_) => std::fs::remove_dir_all(&directory).is_ok(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(_) => false,
    };
    Ok(json!({
        "ok": true,
        "target": target,
        "release": release,
        "session_dir_removed": session_dir_removed,
    }))
}

/// Build the current producer package from one active Attempt.
///
/// Unlike the historical Receipt author, this function has no policy-lane
/// fields and cannot report verification. Producer checks remain explicitly
/// producer-reported inside the signed Submission.
#[allow(clippy::too_many_arguments)]
pub(crate) fn author_submission(
    frontier: &Path,
    actor: &str,
    requested_attempt: Option<&str>,
    assertion: String,
    claim_type: String,
    conditions: Vec<String>,
    replayability: String,
    artifact_flags: &[String],
    caveats: Vec<String>,
    producer_checks: Vec<String>,
    verification_requirements: Vec<String>,
    execution_binding: Option<vela_protocol::receipt_v1::ExecutionBindingV1>,
) -> Result<SubmissionV1, String> {
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};

    if !(actor.starts_with("agent:") || actor.starts_with("ci:")) {
        return Err("Submission authoring requires an agent: or ci: producer".to_string());
    }
    let journal_dir = frontier_transaction_journal_dir(frontier)?;
    let (_migrated, _write_authorization) = acquire_work_write_barrier(frontier, &journal_dir)?;
    let work = resolve_attempt(frontier, actor, requested_attempt)?;
    let mut artifacts = Vec::new();
    let mut total_artifact_bytes = 0_u64;
    for (index, flag) in artifact_flags.iter().enumerate() {
        let (path, kind) = if frontier.join(flag).is_file() {
            (flag.as_str(), "other")
        } else {
            flag.rsplit_once(':').unwrap_or((flag.as_str(), "other"))
        };
        let relative = Path::new(path);
        if relative.is_absolute()
            || !relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)))
        {
            return Err(format!(
                "artifact {index} must be a normalized frontier-relative file"
            ));
        }
        let label = format!("artifact {index}");
        let read_limit = public_artifact_read_limit(total_artifact_bytes, index)?;
        let bytes =
            crate::bounded_file::read_bounded_frontier_file(frontier, relative, read_limit, &label)
                .map_err(|error| public_artifact_read_error(error, read_limit, index))?;
        account_public_artifact_bytes(&mut total_artifact_bytes, bytes.len() as u64, index)?;
        artifacts.push(SubmissionArtifact {
            kind: kind.to_string(),
            path: path.to_string(),
            digest: format!("sha256:{}", hex::encode(Sha256::digest(&bytes))),
        });
    }
    let checks = producer_checks
        .into_iter()
        .map(|value| {
            let (method, outcome) = value.rsplit_once(':').ok_or_else(|| {
                "producer checks use --check <method>:<pass|fail|error|skipped|unknown>".to_string()
            })?;
            ProducerCheck::new(method.to_string(), outcome.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let key = vela_edge::vela_agent_mcp::agent_signing_key(Some(actor))?;
    let identity = IdentityBinding::build(
        IdentityBindingDraft {
            actor_id: actor.to_string(),
            actor_class: ActorClass::Agent,
            created_at: work.record.created_at.clone(),
        },
        &key,
    )?;
    SubmissionV1::build(
        SubmissionDraft {
            claim: SubmissionClaim {
                assertion,
                claim_type,
                conditions,
            },
            artifacts,
            caveats,
            replayability,
            producer_checks: checks,
            verification_requirements,
            requested_change: RequestedChange {
                kind: "add_claim".to_string(),
                target: None,
            },
            provenance: SubmissionProvenance {
                producer: actor.to_string(),
                source_system: "vela-cli".to_string(),
                source_attempt: Some(work.record.attempt_id),
                source_run: None,
                emitted_at: work.record.created_at,
            },
            execution_binding,
        },
        identity,
        &key,
    )
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubmitOutcome {
    pub schema: &'static str,
    pub operation_id: String,
    pub submission_id: String,
    pub submission_root: String,
    pub registration_record_id: String,
    pub registration_record_root: String,
    pub proposal_id: String,
    pub claim_id: String,
    pub route: &'static str,
    pub accepted_event_count_before: usize,
    pub accepted_event_count_after: usize,
    pub accepted_event_delta: usize,
    pub accepted_state_changed: bool,
    pub publication: crate::config::git_publish::PublicationOutcome,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct VerificationImportOutcome {
    pub schema: &'static str,
    pub operation_id: String,
    pub verification_record_id: String,
    pub verification_record_root: String,
    pub proposal_id: String,
    pub claim_id: String,
    pub outcome: String,
    pub idempotent: bool,
    pub accepted_event_delta: usize,
    pub publication: crate::config::git_publish::PublicationOutcome,
}

#[derive(Debug)]
struct PreparedSubmissionArtifacts {
    writes: Vec<crate::frontier_txn::PlannedWrite>,
    read_set: Vec<crate::frontier_txn::InputBinding>,
}

fn prepare_submission_artifacts(
    frontier: &Path,
    submission: &SubmissionV1,
    bundle_root: Option<&Path>,
) -> Result<PreparedSubmissionArtifacts, String> {
    use crate::frontier_txn::{ContentDigest, InputBinding, PlannedWrite, RepoPath, WriteClass};

    let mut blobs = BTreeMap::<String, Vec<u8>>::new();
    let mut read_set = Vec::new();
    let mut total = 0_u64;
    for (index, artifact) in submission.artifacts.iter().enumerate() {
        let relative = Path::new(&artifact.path);
        if relative.is_absolute()
            || !relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)))
        {
            return Err(format!(
                "Submission artifact {index} must be a normalized frontier-relative file"
            ));
        }
        let limit = public_artifact_read_limit(total, index)?;
        let declared_hex = artifact
            .digest
            .strip_prefix("sha256:")
            .ok_or_else(|| format!("Submission artifact {index} digest is not sha256"))?;
        let canonical_path = format!("records/artifacts/sha256/{declared_hex}");
        let canonical_relative = Path::new(&canonical_path);
        let canonical_target = frontier.join(canonical_relative);
        let bytes = if canonical_target.exists() {
            let bytes = crate::bounded_file::read_bounded_frontier_file(
                frontier,
                canonical_relative,
                limit,
                &format!("Submission artifact {index}"),
            )
            .map_err(|error| public_artifact_read_error(error, limit, index))?;
            let tracked = std::process::Command::new("git")
                .current_dir(frontier)
                .args(["ls-files", "--error-unmatch", "--", &canonical_path])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map_err(|error| format!("inspect canonical Artifact tracking: {error}"))?
                .success();
            if !tracked {
                return Err(format!(
                    "Submission artifact {index} already occupies its canonical path but is untracked; remove it and keep the transport blob beside submission.json under artifacts/sha256/{declared_hex}"
                ));
            }
            bytes
        } else if relative == canonical_relative {
            let root = bundle_root.ok_or_else(|| {
                format!(
                    "Submission artifact {index} is absent; place its transport blob beside submission.json under artifacts/sha256/{declared_hex}"
                )
            })?;
            let canonical_root = root
                .canonicalize()
                .map_err(|error| format!("canonicalize Submission transport root: {error}"))?;
            let transport_directory = canonical_root.join("artifacts").join("sha256");
            let source = transport_directory.join(declared_hex);
            let metadata = std::fs::symlink_metadata(&source).map_err(|error| {
                format!("inspect Submission transport artifact {index}: {error}")
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "Submission transport artifact {index} must be a regular non-symlink file"
                ));
            }
            let canonical_source = source.canonicalize().map_err(|error| {
                format!("canonicalize Submission transport artifact {index}: {error}")
            })?;
            if canonical_source != source || !canonical_source.starts_with(&transport_directory) {
                return Err(format!(
                    "Submission transport artifact {index} escapes its canonical bundle directory"
                ));
            }
            crate::bounded_file::read_bounded_file(
                &source,
                limit,
                &format!("Submission transport artifact {index}"),
            )
            .map_err(|error| public_artifact_read_error(error, limit, index))?
        } else {
            crate::bounded_file::read_bounded_frontier_file(
                frontier,
                relative,
                limit,
                &format!("Submission artifact {index}"),
            )
            .map_err(|error| public_artifact_read_error(error, limit, index))?
        };
        account_public_artifact_bytes(&mut total, bytes.len() as u64, index)?;
        let observed = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
        if observed != artifact.digest {
            return Err(format!(
                "Submission artifact {index} digest mismatch: declared {}, observed {observed}",
                artifact.digest
            ));
        }
        if !canonical_target.exists() {
            blobs.entry(canonical_path).or_insert(bytes);
        }
        read_set.push(InputBinding {
            name: format!("submission_artifact[{index}]"),
            digest: ContentDigest::parse(observed).map_err(|error| error.to_string())?,
        });
    }
    let writes = blobs
        .into_iter()
        .map(|(path, bytes)| {
            Ok(PlannedWrite::write(
                RepoPath::parse(path)?,
                WriteClass::CanonicalEvidence,
                bytes,
            ))
        })
        .collect::<Result<Vec<_>, crate::frontier_txn::FrontierTxnError>>()
        .map_err(|error| error.to_string())?;
    Ok(PreparedSubmissionArtifacts { writes, read_set })
}

fn submission_publication_inputs(
    frontier: &Path,
    submission: &SubmissionV1,
) -> Result<Vec<PathBuf>, String> {
    let canonical_frontier = frontier
        .canonicalize()
        .map_err(|error| format!("canonicalize frontier: {error}"))?;
    let mut inputs = submission
        .artifacts
        .iter()
        .map(|artifact| PathBuf::from(&artifact.path))
        .filter(|relative| {
            !relative.is_absolute()
                && relative
                    .components()
                    .all(|component| matches!(component, std::path::Component::Normal(_)))
        })
        .filter(|relative| {
            let lexical = canonical_frontier.join(relative);
            std::fs::symlink_metadata(&lexical)
                .ok()
                .is_some_and(|metadata| metadata.file_type().is_file())
                && lexical
                    .canonicalize()
                    .ok()
                    .is_some_and(|canonical| canonical == lexical)
        })
        .collect::<Vec<_>>();
    inputs.sort();
    inputs.dedup();
    Ok(inputs)
}

fn submission_attempt_close(
    frontier: &Path,
    submission: &SubmissionV1,
    requested_attempt: Option<&str>,
) -> Result<Option<crate::frontier_txn::RepoPath>, String> {
    let Some(attempt_id) = requested_attempt else {
        return Ok(None);
    };
    let resolved = resolve_attempt(frontier, &submission.provenance.producer, Some(attempt_id))?;
    if submission.provenance.source_attempt.as_deref() != Some(&resolved.record.attempt_id) {
        return Err(
            "Submission provenance.source_attempt does not name the selected active Attempt".into(),
        );
    }
    crate::frontier_txn::RepoPath::parse(format!("{}/attempt.json", resolved.relative_dir))
        .map(Some)
        .map_err(|error| error.to_string())
}

/// Register one authenticated Submission through current repository authority.
///
/// This writer has one route: pending review. It writes no Receipt,
/// ActivityRecord, Verification Record, Event, or accepted-state mutation.
pub(crate) fn submit(
    frontier: &Path,
    submission: &SubmissionV1,
    executor: &str,
    requested_attempt: Option<&str>,
    bundle_root: Option<&Path>,
    push: bool,
) -> Result<SubmitOutcome, String> {
    use crate::config::git_publish::{
        PublicationOutcome, PublicationState, PublishOptions, exact_publication_preflight,
        publication_disabled_reason, publication_is_busy, publish_exact_delta,
    };
    use crate::frontier_txn::{ContentDigest, InputBinding, PlannedWrite, WriteClass};

    submission.verify()?;
    let executor = executor.trim();
    if executor != submission.provenance.producer
        || executor != submission.authentication.identity_binding.actor_id
    {
        return Err("submit actor must match the Submission producer identity".to_string());
    }
    let observed = repo::load_from_path(frontier)?;
    let frontier_id = observed.frontier_id().to_string();
    let submission_bytes = submission.canonical_bytes()?;
    let submission_root = submission.canonical_root()?;
    let submission_hex = submission_root
        .strip_prefix("sha256:")
        .ok_or_else(|| "Submission root is not a canonical sha256 digest".to_string())?;
    let submission_path = format!("records/submissions/sha256/{submission_hex}.json");
    let request_bytes = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.submit-request.internal.v1",
        "frontier_id": frontier_id,
        "executor": executor,
        "submission_root": submission_root,
    }))?;
    let request_root = ContentDigest::hash(&request_bytes);
    let operation_id =
        crate::frontier_txn::OperationId::derive("submit", request_root.as_str().as_bytes());
    let journal_dir = frontier_transaction_journal_dir(frontier)?;
    let (repository_authority_enabled, write_barrier) =
        acquire_work_write_barrier(frontier, &journal_dir)?;
    if !repository_authority_enabled {
        return Err(
            "current Submission registration requires repository authority; on a fresh Frontier run `vela authority init . --reason <bounded-reason> --json`"
                .to_string(),
        );
    }
    let (original, repository_authority) = load_project_with_repository_authority(frontier)?;
    let authority = repository_authority.ok_or_else(|| {
        "repository authority disappeared while acquiring the submit barrier".to_string()
    })?;
    if original.frontier_id() != frontier_id {
        return Err("frontier identity changed while acquiring the submit barrier".to_string());
    }
    let registration_action = if authority
        .policy_material
        .schema
        .contains("action \"submission_register\"")
    {
        "submission_register"
    } else if authority
        .policy_material
        .schema
        .contains("action \"receipt_land\"")
    {
        // Read-only compatibility with already-issued repository policy
        // bundles. The current writer still emits only Submission-era objects.
        "receipt_land"
    } else {
        return Err(
            "repository authority does not permit authenticated producer registration".to_string(),
        );
    };
    let fixed_time = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let PreparedSubmissionArtifacts {
        writes: artifact_writes,
        mut read_set,
    } = prepare_submission_artifacts(frontier, submission, bundle_root)?;
    read_set.push(InputBinding {
        name: "submission".to_string(),
        digest: ContentDigest::parse(submission_root.clone()).map_err(|error| error.to_string())?,
    });
    if let Some(target) = submission.requested_change.target.as_ref() {
        read_set.push(InputBinding {
            name: format!("requested_change_target:{}", target.claim_id),
            digest: ContentDigest::parse(target.claim_root.clone())
                .map_err(|error| error.to_string())?,
        });
    }
    let proposal = crate::cli::records::proposal_for_submission(
        &original,
        submission,
        &submission_root,
        &submission_path,
        operation_id.as_str(),
        &fixed_time,
    )?;
    let claim_id = proposal.target.id.clone();
    let proposal_id = proposal.id.clone();
    let materialization_candidate =
        repository_submission_materialization_candidate(frontier, proposal.clone())?;
    let scientific_event_root = authority.verification.final_event_log_root.clone();
    let proposal_after = format!(
        "sha256:{}",
        vela_protocol::proposals::proposal_state_hash(&materialization_candidate.proposals)
    );
    let registration = vela_protocol::registration_record::RegistrationRecordV1::build(
        frontier_id.clone(),
        submission.submission_id.clone(),
        submission_root.clone(),
        submission_path.clone(),
        fixed_time.clone(),
        format!("vela-cli@{}", env!("CARGO_PKG_VERSION")),
        submission
            .authentication
            .identity_binding
            .binding_id
            .clone(),
        Vec::new(),
        claim_id.clone(),
        proposal_id.clone(),
        "pending_review".to_string(),
        request_root.as_str().to_string(),
        vela_protocol::registration_record::RegistrationRoots {
            event_log_before: scientific_event_root.clone(),
            event_log_after: scientific_event_root,
            proposal_after,
        },
        false,
    )?;
    let registration_root = registration.canonical_root()?;
    let registration_hex = registration_root
        .strip_prefix("sha256:")
        .expect("Registration Record root is canonical");
    let registration_path = format!("records/registrations/sha256/{registration_hex}.json");

    let mut managed = repo::render_vela_repo_files(frontier, &materialization_candidate)?;
    preserve_existing_event_bytes(
        frontier,
        &materialization_candidate,
        &materialization_candidate,
        &mut managed,
        "submit",
    )?;
    let mut derived_drafts = Vec::new();
    for write in PlannedWrite::from_managed_files(managed).map_err(|error| error.to_string())? {
        if write.class() != WriteClass::Derived {
            continue;
        }
        let (path, class, postimage) = write
            .into_authority_object_parts()
            .map_err(|error| error.to_string())?;
        if class == WriteClass::Derived
            && crate::authority_transaction::authority_derived_path(&path)
        {
            derived_drafts
                .push(crate::authority_transaction::AuthorityDerivedDraft { path, postimage });
        }
    }
    let mut object_drafts = vec![
        crate::authority_transaction::AuthorityObjectDraft {
            path: format!(".vela/proposals/{proposal_id}.json"),
            object_kind: "proposal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(vela_protocol::canonical::to_canonical_bytes(&proposal)?),
        },
        crate::authority_transaction::AuthorityObjectDraft {
            path: submission_path.clone(),
            object_kind: "submission".into(),
            class: WriteClass::PublicReview,
            postimage: Some(submission_bytes),
        },
        crate::authority_transaction::AuthorityObjectDraft {
            path: registration_path,
            object_kind: "registration_record".into(),
            class: WriteClass::PublicReview,
            postimage: Some(vela_protocol::canonical::to_canonical_bytes(&registration)?),
        },
    ];
    for write in artifact_writes {
        let (path, class, postimage) = write
            .into_authority_object_parts()
            .map_err(|error| error.to_string())?;
        object_drafts.push(crate::authority_transaction::AuthorityObjectDraft {
            path,
            object_kind: "submission_artifact".into(),
            class,
            postimage,
        });
    }
    let WorkWriteBarrier::Repository(recovery_barrier) = write_barrier else {
        return Err("submit lost its repository-authority write barrier".to_string());
    };
    let work_session_close = submission_attempt_close(frontier, submission, requested_attempt)?;
    let authorization_input = CedarEvaluationInput {
        schema: authority.policy_material.schema.clone(),
        policies: authority.policy_material.policies.clone(),
        entities: authority.policy_material.entities.clone(),
        principal: format!(
            "Agent::{}",
            serde_json::to_string(executor).expect("serializing an actor ID cannot fail")
        ),
        principal_class: PrincipalClass::Agent,
        action: registration_action.into(),
        resource: format!(
            "Frontier::{}",
            serde_json::to_string(&frontier_id).expect("serializing a frontier ID cannot fail")
        ),
        context: json!({"exact": true}),
    };
    let (key_id, public_key) = active_repository_signing_key(&authority)?;
    let mut repository_signer =
        crate::repository_authority_provider::SshAgentRepositoryAuthoritySigner::from_environment(
            key_id,
            &public_key,
        )?;
    let executable =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let binary_sha256 = crate::authority_transaction::execution_binary_sha256(&executable)?;
    let mut authentication = SignedAgentSubmissionSession::from_submission(submission)?;
    let mut prepared = crate::authority_transaction::prepare_authority_transaction(
        recovery_barrier,
        frontier,
        crate::authority_transaction::AuthorityTransactionRequest {
            history: authority.history,
            intent_digest: request_root.as_str().to_string(),
            principal: PrincipalSnapshotV1 {
                principal_id: executor.to_string(),
                principal_class: PrincipalClass::Agent,
                display_name: None,
                affiliation: None,
                account_links: vec![executor.to_string()],
            },
            authentication_request: AuthenticationRequest {
                principal_id: executor.to_string(),
                principal_class: PrincipalClass::Agent,
                transaction_at: fixed_time,
            },
            runtime_session_state: RuntimeSessionState::default(),
            authorization_input,
            delegation: None,
            semantic_approvals: Vec::new(),
            event_drafts: Vec::new(),
            object_drafts,
            derived_drafts,
            next_authority_keyset: None,
            next_policy_bundle: None,
            next_policy_material: None,
            read_set,
            vela_version: env!("CARGO_PKG_VERSION").into(),
            binary_sha256,
            recorded_at: registration.registered_at.clone(),
        },
        &mut authentication,
        &mut repository_signer,
    )
    .map_err(|error| error.to_string())?;
    let public = prepared
        .resolved_public_writes()
        .map_err(|error| error.to_string())?;
    let delta_root = prepared.canonical_delta_root().to_string();
    let mut publish_opts = if push {
        PublishOptions::pushing()
    } else {
        PublishOptions::new(false)
    };
    let publication_disabled = publication_disabled_reason(frontier, &publish_opts);
    if publication_disabled.is_none() {
        publish_opts = publish_opts
            .with_preflight_inputs(submission_publication_inputs(frontier, submission)?);
    }
    let publication_delta = if publication_disabled.is_some() {
        None
    } else {
        publication_delta(frontier, &delta_root, public)?
    };
    let publication_preflight = publication_delta
        .as_ref()
        .map(|delta| exact_publication_preflight(frontier, delta, &publish_opts))
        .transpose();
    let publication_preflight = match publication_preflight {
        Ok(value) => value,
        Err(outcome) if publication_is_busy(&outcome) => {
            return Err(
                "another Vela write/publication owns this repository; Submission was not registered"
                    .to_string(),
            );
        }
        Err(outcome) => {
            prepared
                .mark_committed()
                .map_err(|error| error.to_string())?;
            prepared.install().map_err(|error| error.to_string())?;
            prepared.complete().map_err(|error| error.to_string())?;
            if let Some(path) = work_session_close {
                let _ = std::fs::remove_file(frontier.join(path.as_str()));
            }
            return Ok(SubmitOutcome {
                schema: "vela.submit-result.v1",
                operation_id: operation_id.as_str().to_string(),
                submission_id: submission.submission_id.clone(),
                submission_root,
                registration_record_id: registration.registration_record_id,
                registration_record_root: registration_root,
                proposal_id,
                claim_id,
                route: "pending_review",
                accepted_event_count_before: original.events.len(),
                accepted_event_count_after: original.events.len(),
                accepted_event_delta: 0,
                accepted_state_changed: false,
                publication: outcome,
            });
        }
    };
    prepared
        .mark_committed()
        .map_err(|error| error.to_string())?;
    prepared.install().map_err(|error| error.to_string())?;
    prepared.complete().map_err(|error| error.to_string())?;
    if let Some(path) = work_session_close {
        let target = frontier.join(path.as_str());
        match std::fs::remove_file(&target) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Submission registered but Attempt cleanup failed at {}: {error}",
                    target.display()
                ));
            }
        }
    }
    let publication = match (publication_delta.as_ref(), publication_preflight) {
        (Some(delta), Some(preflight)) => publish_exact_delta(
            frontier,
            "submit",
            std::slice::from_ref(&proposal_id),
            delta,
            preflight,
            &publish_opts,
        )
        .unwrap_or_else(|error| PublicationOutcome {
            state: PublicationState::Unknown {
                reason: error.to_string(),
            },
            recovery_command: None,
        }),
        _ => PublicationOutcome {
            state: PublicationState::Uncommitted {
                candidate: None,
                reason: publication_disabled.unwrap_or_else(|| {
                    "Submission transaction had no public Git delta".to_string()
                }),
            },
            recovery_command: None,
        },
    };
    Ok(SubmitOutcome {
        schema: "vela.submit-result.v1",
        operation_id: operation_id.as_str().to_string(),
        submission_id: submission.submission_id.clone(),
        submission_root,
        registration_record_id: registration.registration_record_id,
        registration_record_root: registration_root,
        proposal_id,
        claim_id,
        route: "pending_review",
        accepted_event_count_before: original.events.len(),
        accepted_event_count_after: original.events.len(),
        accepted_event_delta: 0,
        accepted_state_changed: false,
        publication,
    })
}

/// Retain one authenticated Verification Record through repository authority.
///
/// Verification remains scoped evidence. Import writes no scientific Event,
/// changes no Proposal standing, and cannot accept a Claim.
pub(crate) fn import_verification(
    frontier: &Path,
    record: &vela_protocol::verification_record::VerificationRecordV1,
    executor: &str,
    push: bool,
) -> Result<VerificationImportOutcome, String> {
    use crate::config::git_publish::{
        PublicationOutcome, PublicationState, PublishOptions, exact_publication_preflight,
        publication_disabled_reason, publication_is_busy, publish_exact_delta,
    };
    use crate::frontier_txn::{ContentDigest, InputBinding, WriteClass};

    record.verify()?;
    let executor = executor.trim();
    if executor != record.verifier || executor != record.authentication.identity_binding.actor_id {
        return Err("verification import actor must match the Verification Record verifier".into());
    }
    let observed = repo::load_from_path(frontier)?;
    let frontier_id = observed.frontier_id().to_string();
    let proposal = observed
        .proposals
        .iter()
        .find(|proposal| proposal.id == record.subject.proposal_id)
        .ok_or_else(|| format!("proposal {} does not exist", record.subject.proposal_id))?;
    if proposal.status != "pending_review" {
        return Err("Verification Records may be imported only for a pending Proposal".into());
    }
    if proposal.target.id != record.subject.claim_id {
        return Err("Verification Record claim does not match the Proposal target".into());
    }
    let submission_link = proposal
        .payload
        .get("submission")
        .and_then(Value::as_object)
        .ok_or("Proposal does not bind a current Submission")?;
    if submission_link.get("submission_id").and_then(Value::as_str)
        != Some(record.subject.submission_id.as_str())
        || submission_link
            .get("submission_root")
            .and_then(Value::as_str)
            != Some(record.subject.submission_root.as_str())
    {
        return Err("Verification Record does not bind the Proposal's exact Submission".into());
    }
    let submission_path = submission_link
        .get("submission_path")
        .and_then(Value::as_str)
        .ok_or("Proposal Submission link has no exact path")?;
    let submission_bytes = crate::bounded_file::read_bounded_file(
        &frontier.join(submission_path),
        8 * 1024 * 1024,
        "Submission",
    )
    .map_err(|error| error.to_string())?;
    let submission = SubmissionV1::parse(&submission_bytes)?;
    if submission.submission_id != record.subject.submission_id
        || submission.canonical_root()? != record.subject.submission_root
    {
        return Err("stored Submission does not match the Verification Record subject".into());
    }

    let record_bytes = record.canonical_bytes()?;
    let record_root = record.canonical_root()?;
    let record_hex = record_root
        .strip_prefix("sha256:")
        .ok_or("Verification Record root is not canonical")?;
    let record_path = format!("records/verifications/sha256/{record_hex}.json");
    let request_bytes = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.verification-import-request.internal.v1",
        "frontier_id": frontier_id,
        "executor": executor,
        "verification_record_root": record_root,
    }))?;
    let request_root = ContentDigest::hash(&request_bytes);
    let operation_id = crate::frontier_txn::OperationId::derive(
        "verification-import",
        request_root.as_str().as_bytes(),
    );

    let existing_path = frontier.join(&record_path);
    if existing_path.exists() {
        let existing = crate::bounded_file::read_bounded_file(
            &existing_path,
            4 * 1024 * 1024,
            "Verification Record",
        )
        .map_err(|error| error.to_string())?;
        if existing != record_bytes {
            return Err("Verification Record path exists with different bytes".into());
        }
        return Ok(VerificationImportOutcome {
            schema: "vela.verification-import-result.v1",
            operation_id: operation_id.as_str().to_string(),
            verification_record_id: record.verification_record_id.clone(),
            verification_record_root: record_root,
            proposal_id: record.subject.proposal_id.clone(),
            claim_id: record.subject.claim_id.clone(),
            outcome: record.outcome.clone(),
            idempotent: true,
            accepted_event_delta: 0,
            publication: PublicationOutcome {
                state: PublicationState::Uncommitted {
                    candidate: None,
                    reason: "exact Verification Record is already registered".into(),
                },
                recovery_command: None,
            },
        });
    }

    let journal_dir = frontier_transaction_journal_dir(frontier)?;
    let (repository_authority_enabled, write_barrier) =
        acquire_work_write_barrier(frontier, &journal_dir)?;
    if !repository_authority_enabled {
        return Err(
            "current Verification Record import requires repository authority; on a fresh Frontier run `vela authority init . --reason <bounded-reason> --json`"
                .into(),
        );
    }
    let (current, repository_authority) = load_project_with_repository_authority(frontier)?;
    let authority = repository_authority
        .ok_or("repository authority disappeared while acquiring the verification barrier")?;
    if current.frontier_id() != frontier_id
        || format!(
            "sha256:{}",
            vela_protocol::proposals::proposal_state_hash(&current.proposals)
        ) != format!(
            "sha256:{}",
            vela_protocol::proposals::proposal_state_hash(&observed.proposals)
        )
    {
        return Err("frontier or Proposal state changed before verification import".into());
    }
    if !authority
        .policy_material
        .schema
        .contains("action \"verification_import\"")
    {
        return Err(
            "repository authority does not permit Verification Record import; rotate to the current routine-work policy"
                .into(),
        );
    }
    let WorkWriteBarrier::Repository(recovery_barrier) = write_barrier else {
        return Err("verification import lost its repository-authority barrier".into());
    };
    let recorded_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let proposal_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(proposal)?
    );
    let read_set = vec![
        InputBinding {
            name: "verification_record".into(),
            digest: ContentDigest::parse(record_root.clone()).map_err(|error| error.to_string())?,
        },
        InputBinding {
            name: "submission".into(),
            digest: ContentDigest::parse(record.subject.submission_root.clone())
                .map_err(|error| error.to_string())?,
        },
        InputBinding {
            name: "proposal".into(),
            digest: ContentDigest::parse(proposal_root).map_err(|error| error.to_string())?,
        },
    ];
    let authorization_input = CedarEvaluationInput {
        schema: authority.policy_material.schema.clone(),
        policies: authority.policy_material.policies.clone(),
        entities: authority.policy_material.entities.clone(),
        principal: format!(
            "Agent::{}",
            serde_json::to_string(executor).expect("actor serialization cannot fail")
        ),
        principal_class: PrincipalClass::Agent,
        action: "verification_import".into(),
        resource: format!(
            "Frontier::{}",
            serde_json::to_string(&frontier_id).expect("frontier serialization cannot fail")
        ),
        context: json!({"exact": true}),
    };
    let (key_id, public_key) = active_repository_signing_key(&authority)?;
    let mut repository_signer =
        crate::repository_authority_provider::SshAgentRepositoryAuthoritySigner::from_environment(
            key_id,
            &public_key,
        )?;
    let executable =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let binary_sha256 = crate::authority_transaction::execution_binary_sha256(&executable)?;
    let mut authentication = SignedVerificationRecordSession::from_record(record)?;
    let mut prepared = crate::authority_transaction::prepare_authority_transaction(
        recovery_barrier,
        frontier,
        crate::authority_transaction::AuthorityTransactionRequest {
            history: authority.history,
            intent_digest: request_root.as_str().to_string(),
            principal: PrincipalSnapshotV1 {
                principal_id: executor.to_string(),
                principal_class: PrincipalClass::Agent,
                display_name: None,
                affiliation: None,
                account_links: vec![executor.to_string()],
            },
            authentication_request: AuthenticationRequest {
                principal_id: executor.to_string(),
                principal_class: PrincipalClass::Agent,
                transaction_at: recorded_at.clone(),
            },
            runtime_session_state: RuntimeSessionState::default(),
            authorization_input,
            delegation: None,
            semantic_approvals: Vec::new(),
            event_drafts: Vec::new(),
            object_drafts: vec![crate::authority_transaction::AuthorityObjectDraft {
                path: record_path,
                object_kind: "verification_record".into(),
                class: WriteClass::PublicReview,
                postimage: Some(record_bytes),
            }],
            derived_drafts: Vec::new(),
            next_authority_keyset: None,
            next_policy_bundle: None,
            next_policy_material: None,
            read_set,
            vela_version: env!("CARGO_PKG_VERSION").into(),
            binary_sha256,
            recorded_at,
        },
        &mut authentication,
        &mut repository_signer,
    )
    .map_err(|error| error.to_string())?;
    let public = prepared
        .resolved_public_writes()
        .map_err(|error| error.to_string())?;
    let delta_root = prepared.canonical_delta_root().to_string();
    let publish_opts = if push {
        PublishOptions::pushing()
    } else {
        PublishOptions::new(false)
    };
    let publication_disabled = publication_disabled_reason(frontier, &publish_opts);
    let publication_delta = if publication_disabled.is_some() {
        None
    } else {
        publication_delta(frontier, &delta_root, public)?
    };
    let publication_preflight = publication_delta
        .as_ref()
        .map(|delta| exact_publication_preflight(frontier, delta, &publish_opts))
        .transpose();
    let publication_preflight = match publication_preflight {
        Ok(value) => value,
        Err(outcome) if publication_is_busy(&outcome) => {
            return Err(
                "another Vela write/publication owns this repository; Verification Record was not imported"
                    .into(),
            );
        }
        Err(outcome) => {
            prepared
                .mark_committed()
                .map_err(|error| error.to_string())?;
            prepared.install().map_err(|error| error.to_string())?;
            prepared.complete().map_err(|error| error.to_string())?;
            return Ok(VerificationImportOutcome {
                schema: "vela.verification-import-result.v1",
                operation_id: operation_id.as_str().to_string(),
                verification_record_id: record.verification_record_id.clone(),
                verification_record_root: record_root,
                proposal_id: record.subject.proposal_id.clone(),
                claim_id: record.subject.claim_id.clone(),
                outcome: record.outcome.clone(),
                idempotent: false,
                accepted_event_delta: 0,
                publication: outcome,
            });
        }
    };
    prepared
        .mark_committed()
        .map_err(|error| error.to_string())?;
    prepared.install().map_err(|error| error.to_string())?;
    prepared.complete().map_err(|error| error.to_string())?;
    let publication = match (publication_delta.as_ref(), publication_preflight) {
        (Some(delta), Some(preflight)) => publish_exact_delta(
            frontier,
            "verification import",
            std::slice::from_ref(&record.verification_record_id),
            delta,
            preflight,
            &publish_opts,
        )
        .unwrap_or_else(|error| PublicationOutcome {
            state: PublicationState::Unknown {
                reason: error.to_string(),
            },
            recovery_command: None,
        }),
        _ => PublicationOutcome {
            state: PublicationState::Uncommitted {
                candidate: None,
                reason: publication_disabled
                    .unwrap_or_else(|| "Verification import had no public Git delta".into()),
            },
            recovery_command: None,
        },
    };
    Ok(VerificationImportOutcome {
        schema: "vela.verification-import-result.v1",
        operation_id: operation_id.as_str().to_string(),
        verification_record_id: record.verification_record_id.clone(),
        verification_record_root: record_root,
        proposal_id: record.subject.proposal_id.clone(),
        claim_id: record.subject.claim_id.clone(),
        outcome: record.outcome.clone(),
        idempotent: false,
        accepted_event_delta: 0,
        publication,
    })
}

/// Worktree-private recovery storage for scientific frontier transactions.
///
/// Git publication keeps its transport journal inside the Git directory, but
/// scientific state must remain usable when publication is unavailable or a
/// frontier was deliberately initialized without Git. These bytes are ignored
/// scratch, never replay or authority state.
pub(crate) fn frontier_transaction_journal_dir(frontier: &Path) -> Result<PathBuf, String> {
    let root = frontier
        .canonicalize()
        .map_err(|error| format!("resolve frontier transaction root: {error}"))?;
    let vela = root.join(".vela");
    let metadata = std::fs::symlink_metadata(&vela).map_err(|error| {
        format!(
            "inspect frontier private directory {}: {error}",
            vela.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "frontier private directory must be a real directory: {}",
            vela.display()
        ));
    }
    let journal = vela.join("operation-journals");
    match std::fs::symlink_metadata(&journal) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(format!(
            "frontier transaction journal must be a real directory: {}",
            journal.display()
        )),
        Ok(_) => Ok(journal),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(journal),
        Err(error) => Err(format!(
            "inspect frontier transaction journal {}: {error}",
            journal.display()
        )),
    }
}

fn account_public_artifact_bytes(
    total: &mut u64,
    artifact_bytes: u64,
    index: usize,
) -> Result<(), String> {
    let next = total.checked_add(artifact_bytes).ok_or_else(|| {
        format!("public artifact byte count overflowed while reading artifact {index}")
    })?;
    if next > crate::bounded_file::PUBLIC_ARTIFACT_TOTAL_MAX_BYTES {
        return Err(format!(
            "public artifacts exceed the {}-byte total limit at artifact {index}",
            crate::bounded_file::PUBLIC_ARTIFACT_TOTAL_MAX_BYTES
        ));
    }
    *total = next;
    Ok(())
}

fn public_artifact_read_limit(total: u64, index: usize) -> Result<u64, String> {
    let remaining = crate::bounded_file::PUBLIC_ARTIFACT_TOTAL_MAX_BYTES
        .checked_sub(total)
        .ok_or_else(|| {
            format!(
                "public artifacts already exceed the {}-byte total limit before artifact {index}",
                crate::bounded_file::PUBLIC_ARTIFACT_TOTAL_MAX_BYTES
            )
        })?;
    Ok(remaining.min(crate::bounded_file::PUBLIC_ARTIFACT_MAX_BYTES))
}

fn public_artifact_read_error(
    error: crate::bounded_file::BoundedFileError,
    read_limit: u64,
    index: usize,
) -> String {
    if error.code == "oversized" && read_limit < crate::bounded_file::PUBLIC_ARTIFACT_MAX_BYTES {
        format!(
            "public artifacts exceed the {}-byte total limit at artifact {index}",
            crate::bounded_file::PUBLIC_ARTIFACT_TOTAL_MAX_BYTES
        )
    } else {
        error.to_string()
    }
}

#[cfg(test)]
mod public_artifact_budget_tests {
    use std::path::PathBuf;

    use super::{
        account_public_artifact_bytes, prepare_submission_artifacts, public_artifact_read_limit,
    };
    use crate::bounded_file::{PUBLIC_ARTIFACT_MAX_BYTES, PUBLIC_ARTIFACT_TOTAL_MAX_BYTES};

    #[test]
    fn public_artifact_total_budget_accepts_the_boundary_and_rejects_overflow() {
        let mut total = PUBLIC_ARTIFACT_TOTAL_MAX_BYTES - 1;
        account_public_artifact_bytes(&mut total, 1, 7).unwrap();
        assert_eq!(total, PUBLIC_ARTIFACT_TOTAL_MAX_BYTES);

        let error = account_public_artifact_bytes(&mut total, 1, 8).unwrap_err();
        assert_eq!(
            error,
            format!(
                "public artifacts exceed the {}-byte total limit at artifact 8",
                PUBLIC_ARTIFACT_TOTAL_MAX_BYTES
            )
        );
        assert_eq!(total, PUBLIC_ARTIFACT_TOTAL_MAX_BYTES);
    }

    #[test]
    fn public_artifact_total_budget_rejects_arithmetic_overflow() {
        let mut total = u64::MAX;
        let error = account_public_artifact_bytes(&mut total, 1, 1).unwrap_err();
        assert_eq!(
            error,
            "public artifact byte count overflowed while reading artifact 1"
        );
        assert_eq!(total, u64::MAX);
    }

    #[test]
    fn public_artifact_reader_never_crosses_the_remaining_total_budget() {
        assert_eq!(
            public_artifact_read_limit(0, 0).unwrap(),
            PUBLIC_ARTIFACT_MAX_BYTES
        );
        assert_eq!(
            public_artifact_read_limit(PUBLIC_ARTIFACT_TOTAL_MAX_BYTES - 1, 8).unwrap(),
            1
        );
        assert_eq!(
            public_artifact_read_limit(PUBLIC_ARTIFACT_TOTAL_MAX_BYTES, 9).unwrap(),
            0
        );
        assert!(
            public_artifact_read_limit(PUBLIC_ARTIFACT_TOTAL_MAX_BYTES + 1, 9)
                .unwrap_err()
                .contains("already exceed")
        );
    }

    #[test]
    fn foreign_submission_reads_transport_blob_without_precopying_canonical_path() {
        let fixtures =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/current-objects");
        let submission = vela_protocol::submission_v1::SubmissionV1::parse(
            &std::fs::read(fixtures.join("submission.json")).unwrap(),
        )
        .unwrap();
        let frontier = tempfile::tempdir().unwrap();
        let project =
            vela_protocol::project::assemble("transport-fixture", Vec::new(), 0, 0, "fixture");
        vela_protocol::repo::init_repo(frontier.path(), &project).unwrap();

        let prepared =
            prepare_submission_artifacts(frontier.path(), &submission, Some(&fixtures)).unwrap();
        assert_eq!(prepared.writes.len(), 1);
        assert_eq!(prepared.read_set.len(), 1);
        let (path, class, postimage) = prepared.writes[0]
            .clone()
            .into_authority_object_parts()
            .unwrap();
        assert_eq!(
            path,
            "records/artifacts/sha256/084c799cd551dd1d8d5c5f9a5d593b2e931f5e36122ee5c793c1d08a19839cc0"
        );
        assert_eq!(class, crate::frontier_txn::WriteClass::CanonicalEvidence);
        assert_eq!(postimage.unwrap(), b"42\n");
        assert!(!frontier.path().join(path).exists());
    }

    #[cfg(unix)]
    #[test]
    fn foreign_submission_rejects_symlinked_transport_ancestors() {
        use std::os::unix::fs::symlink;

        let fixtures =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/current-objects");
        let submission = vela_protocol::submission_v1::SubmissionV1::parse(
            &std::fs::read(fixtures.join("submission.json")).unwrap(),
        )
        .unwrap();
        let frontier = tempfile::tempdir().unwrap();
        let project =
            vela_protocol::project::assemble("transport-hostile", Vec::new(), 0, 0, "fixture");
        vela_protocol::repo::init_repo(frontier.path(), &project).unwrap();
        let bundle = tempfile::tempdir().unwrap();
        symlink(fixtures.join("artifacts"), bundle.path().join("artifacts")).unwrap();

        let error = prepare_submission_artifacts(frontier.path(), &submission, Some(bundle.path()))
            .unwrap_err();
        assert!(error.contains("escapes its canonical bundle directory"));
    }
}

fn publication_delta(
    frontier: &Path,
    root: &str,
    writes: Vec<crate::frontier_txn::ResolvedWrite>,
) -> Result<Option<crate::config::git_publish::PublicationDelta>, String> {
    use crate::config::git_publish::{PublicationDelta, PublicationDeltaEntry};
    use crate::frontier_txn::{FileMode, FileState};
    if writes.is_empty() {
        return Ok(None);
    }
    let mut entries = writes
        .into_iter()
        .map(|write| {
            let path = crate::config::git_publish::publication_repo_relative_path(
                frontier,
                write.staged.path.as_str(),
            )?;
            let preimage_sha256 = match &write.staged.preimage {
                FileState::Absent => None,
                FileState::File { digest, .. } => Some(digest.as_str().to_string()),
            };
            let executable = matches!(
                write.staged.postimage,
                FileState::File {
                    mode: FileMode::Executable,
                    ..
                }
            );
            Ok(PublicationDeltaEntry {
                path,
                preimage_sha256,
                postimage: write.postimage_bytes,
                executable,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(Some(PublicationDelta {
        root: root.to_string(),
        entries,
    }))
}

#[cfg(test)]
mod workflow_transaction_tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use vela_protocol::bundle::{
        Assertion, Conditions, Confidence, ConfidenceKind, ConfidenceMethod, Evidence, Extraction,
        FindingBundle, Flags, Provenance,
    };
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::receipt_v1::{ArtifactInput, ReceiptBuilder, ReceiptInput};

    #[test]
    fn repository_submission_materialization_excludes_detached_attempt_overlays() {
        let temp = tempfile::tempdir().unwrap();
        let canonical =
            vela_protocol::project::assemble("materialization-source", Vec::new(), 0, 0, "test");
        vela_protocol::repo::init_repo(temp.path(), &canonical).unwrap();

        // Model the effective workflow view: repository-authority loading may
        // overlay an active detached lease so submit can validate the Attempt
        // session. This state must not be the source for frontier.json or
        // vela.lock.
        let mut effective = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        effective.events.push(vela_protocol::events::StateEvent {
            schema: vela_protocol::events::EVENT_SCHEMA.to_string(),
            id: "vev_detached_lease_overlay".to_string(),
            kind: vela_protocol::events::EVENT_KIND_ATTEMPT_CLAIMED.into(),
            target: vela_protocol::events::StateTarget {
                r#type: "attempt".to_string(),
                id: "seed:target".to_string(),
            },
            actor: vela_protocol::events::StateActor {
                id: "agent:fixture".to_string(),
                r#type: "agent".to_string(),
            },
            timestamp: "2026-07-26T00:00:00Z".to_string(),
            reason: "detached repository-authority lease overlay".to_string(),
            before_hash: vela_protocol::events::NULL_HASH.to_string(),
            after_hash: vela_protocol::events::NULL_HASH.to_string(),
            payload: json!({}),
            caveats: Vec::new(),
            signature: None,
        });
        assert_eq!(effective.events.len(), canonical.events.len() + 1);

        let pending_finding = finding();
        let proposal = vela_protocol::proposals::new_proposal_at(
            "finding.add",
            vela_protocol::events::StateTarget {
                r#type: "finding".to_string(),
                id: pending_finding.id.clone(),
            },
            "agent:fixture",
            "agent",
            "retain verified work for review",
            json!({ "finding": pending_finding }),
            Vec::new(),
            Vec::new(),
            "2026-07-26T00:00:01Z",
        );
        let proposal_id = proposal.id.clone();

        let materialized =
            repository_submission_materialization_candidate(temp.path(), proposal).unwrap();
        assert_eq!(materialized.events.len(), canonical.events.len());
        assert!(
            materialized
                .events
                .iter()
                .all(|event| event.id != "vev_detached_lease_overlay")
        );
        assert!(
            materialized
                .proposals
                .iter()
                .any(|proposal| proposal.id == proposal_id)
        );
    }

    fn review_withdraw_fixture() -> (tempfile::TempDir, SigningKey, String) {
        let temp = tempfile::tempdir().unwrap();
        let key = SigningKey::from_bytes(&[31_u8; 32]);
        let actor = "agent:workflow-withdrawal";
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: actor.to_string(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-17T00:00:00Z".to_string(),
            },
            &key,
        )
        .unwrap();
        let mut project =
            vela_protocol::project::assemble("workflow-withdrawal", Vec::new(), 0, 0, "test");
        vela_protocol::repo::init_repo(temp.path(), &project).unwrap();
        let receipt = ReceiptBuilder::build(
            ReceiptInput::new(
                "bounded workflow result".to_string(),
                "computational".to_string(),
                "exact".to_string(),
                vec![
                    ArtifactInput::new(
                        "artifact.json".to_string(),
                        "witness".to_string(),
                        Some("a".repeat(64)),
                        None,
                    )
                    .unwrap(),
                ],
                vec!["bounded only".to_string()],
                Vec::new(),
                actor.to_string(),
                "2026-07-17T00:00:01Z".to_string(),
                format!(
                    "sha256:{}",
                    vela_protocol::events::event_log_hash(&project.events)
                ),
                ".".to_string(),
                format!("vop_{}", "b".repeat(64)),
                "urn:vela:policy:none".to_string(),
            )
            .unwrap(),
            &identity,
        )
        .unwrap();
        let receipt_root = receipt.canonical_root().unwrap();
        let receipt_path = format!(
            "records/receipts/sha256/{}.json",
            receipt_root.strip_prefix("sha256:").unwrap()
        );
        std::fs::create_dir_all(temp.path().join("records/receipts/sha256")).unwrap();
        std::fs::write(
            temp.path().join(&receipt_path),
            receipt.canonical_bytes().unwrap(),
        )
        .unwrap();
        let proposal = vela_protocol::proposals::new_proposal_at(
            "finding.review",
            vela_protocol::events::StateTarget {
                r#type: "finding".to_string(),
                id: "vf_workflow_target".to_string(),
            },
            actor,
            "agent",
            "land bounded result",
            json!({
                "vela_submission": {
                    "schema": "vela.submission-links.internal.v1",
                    "receipt_root": receipt_root,
                    "receipt_path": receipt_path,
                    "record_id": "vrc_0123456789abcdef",
                    "operation_id": format!("vop_{}", "b".repeat(64)),
                    "review_material_path": "records/review/sha256/test.json",
                }
            }),
            Vec::new(),
            Vec::new(),
            "2026-07-17T00:00:02Z",
        );
        let proposal_id = proposal.id.clone();
        project.proposals.push(proposal);
        vela_protocol::repo::save_to_path(temp.path(), &project).unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.name", "Vela Test"],
            vec!["config", "user.email", "vela-test@example.invalid"],
            vec!["add", "."],
            vec!["commit", "-q", "-m", "fixture"],
        ] {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(temp.path())
                .status()
                .unwrap();
            assert!(status.success());
        }
        (temp, key, proposal_id)
    }

    #[test]
    fn review_withdraw_transaction_is_receipt_bound_and_idempotent_without_second_key_read() {
        let (temp, key, proposal_id) = review_withdraw_fixture();
        let before = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        let before_findings = vela_protocol::canonical::sha256_canonical(&before.findings).unwrap();
        let first = transact_proposal_withdrawal(
            temp.path(),
            &proposal_id,
            "agent:workflow-withdrawal",
            "superseded fixture",
            || Ok(key.clone()),
        )
        .unwrap();
        assert_eq!(first["idempotent"], false);
        assert_eq!(first["key_read"], true);

        let after = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        assert_eq!(after.proposals[0].status, "withdrawn");
        assert_eq!(
            vela_protocol::canonical::sha256_canonical(&after.findings).unwrap(),
            before_findings
        );
        assert!(
            vela_protocol::proposals::verify_proposal_withdrawals(temp.path(), &after).is_empty()
        );
        assert_eq!(after.stats.event_count, before.stats.event_count + 1);
        assert!(
            vela_protocol::frontier_repo::layout_issues(temp.path(), &after).is_empty(),
            "withdrawal transaction must publish derived views that already match replay"
        );

        let retry = transact_proposal_withdrawal(
            temp.path(),
            &proposal_id,
            "agent:workflow-withdrawal",
            "ignored on verified retry",
            || panic!("idempotent withdrawal must not read the producer key"),
        )
        .unwrap();
        assert_eq!(retry["idempotent"], true);
        assert_eq!(retry["key_read"], false);
        assert_eq!(retry["withdrawal_event_id"], first["withdrawal_event_id"]);
    }

    fn attempt_size_fixture(padding: usize) -> Attempt {
        let contract = TaskContract {
            schema: TASK_CONTRACT_SCHEMA.to_string(),
            objective: "fixture".to_string(),
            completion_condition: "fixture".to_string(),
            allowed_actions: Vec::new(),
            forbidden_actions: Vec::new(),
            required_outputs: Vec::new(),
            required_checks: Vec::new(),
            escalation_path: "fixture".to_string(),
            authority_ceiling: PRODUCER_AUTHORITY_CEILING_FOR_TEST.to_string(),
        };
        Attempt {
            schema: ATTEMPT_SCHEMA.to_string(),
            attempt_id: format!("vat_{}", "0".repeat(64)),
            target: "seed:size-fixture".to_string(),
            frontier_id: "vfr_size_fixture".to_string(),
            base_event_log_root: format!("sha256:{}", "0".repeat(64)),
            base_nonlease_event_log_root: format!("sha256:{}", "0".repeat(64)),
            base_authority_nonlease_event_log_root: None,
            source_git_commit_oid: Some("0".repeat(40)),
            source_git_state: "pinned".to_string(),
            actor: "agent:size-fixture".to_string(),
            created_at: "2026-07-14T00:00:00+00:00".to_string(),
            lease: WorkSessionLease {
                claim_event_id: format!("vev_{}", "0".repeat(64)),
                claimant_pubkey: "0".repeat(64),
                claimed_at: "2026-07-14T00:00:00+00:00".to_string(),
                lease_ttl_seconds: 86_400,
                expires_at: "2026-07-15T00:00:00+00:00".to_string(),
            },
            task_contract_root: sha256_root(&contract).unwrap(),
            task_contract: contract,
            submission_builder: SubmissionBuilderAttemptFacts::default(),
            target_task_binding: None,
            briefing: json!({"padding": "x".repeat(padding)}),
        }
    }

    const PRODUCER_AUTHORITY_CEILING_FOR_TEST: &str =
        "Producer evidence only; fixture cannot accept truth.";

    #[test]
    fn target_binding_extends_attempt_identity() {
        let legacy = attempt_id(
            "vfr_1234567890abcdef",
            "erdos:1056",
            "agent:indexed",
            &format!("vev_{}", "1".repeat(64)),
            &format!("sha256:{}", "2".repeat(64)),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            legacy,
            "vat_9be875f8d6fc0dbae603811935f5ba2e2a079ac88654ed1bd1abdfeef8ecfb14"
        );

        let bound = attempt_id(
            "vfr_1234567890abcdef",
            "erdos:1056",
            "agent:indexed",
            &format!("vev_{}", "1".repeat(64)),
            &format!("sha256:{}", "2".repeat(64)),
            Some(&format!("sha256:{}", "3".repeat(64))),
            None,
        )
        .unwrap();
        assert_eq!(
            bound,
            "vat_29a886f89c32ef8a2b13864339f229541a0b2a174bf0afb4f0dea3ac7e34a221"
        );
        assert_ne!(bound, legacy);
    }

    #[test]
    fn attempt_requires_the_nonlease_root_without_a_fallback() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("attempt.json");
        let mut value = serde_json::to_value(attempt_size_fixture(0)).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("base_nonlease_event_log_root");
        std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let error = parse_attempt(&path).unwrap_err();
        assert!(error.contains("base_nonlease_event_log_root"), "{error}");
        assert!(error.contains("rerun `vela start"), "{error}");
    }

    #[test]
    fn attempt_size_ceiling_is_exact_and_preflight_precedes_claim() {
        let empty = attempt_size_fixture(0);
        let empty_len = serde_json::to_vec_pretty(&empty).unwrap().len() + 1;
        assert!(empty_len < ATTEMPT_MAX_BYTES);
        let at_limit = attempt_size_fixture(ATTEMPT_MAX_BYTES - empty_len);
        assert_eq!(encoded_attempt(&at_limit).unwrap().len(), ATTEMPT_MAX_BYTES);
        let over_limit = attempt_size_fixture(ATTEMPT_MAX_BYTES - empty_len + 1);
        assert!(encoded_attempt(&over_limit).unwrap_err().contains("limit"));

        let temp = tempfile::tempdir().unwrap();
        let project =
            vela_protocol::project::assemble("preflight-no-lease", Vec::new(), 0, 0, "fixture");
        vela_protocol::repo::init_repo(temp.path(), &project).unwrap();
        let before = repo::load_from_path(temp.path()).unwrap();
        let oversized_actor = format!("agent:{}", "x".repeat(ATTEMPT_MAX_BYTES));
        let error = open_session(
            temp.path(),
            "seed:oversized-session",
            &oversized_actor,
            86_400,
        )
        .unwrap_err();
        assert!(error.contains("Attempt record is"), "{error}");
        let after = repo::load_from_path(temp.path()).unwrap();
        assert_eq!(
            vela_protocol::events::event_log_hash(&after.events),
            vela_protocol::events::event_log_hash(&before.events)
        );
        assert_eq!(after.attempt_claims, before.attempt_claims);
    }

    fn finding() -> FindingBundle {
        FindingBundle::new(
            Assertion {
                text: "transactional proposal fixture".to_string(),
                assertion_type: "mechanism".to_string(),
                entities: Vec::new(),
                relation: None,
                direction: None,
                causal_claim: None,
                causal_evidence_grade: None,
            },
            Evidence {
                evidence_type: "experimental".to_string(),
                model_system: "fixture".to_string(),
                method: "fixture".to_string(),
                replicated: false,
                replication_count: None,
                evidence_spans: Vec::new(),
            },
            Conditions {
                text: "fixture".to_string(),
                duration: None,
            },
            Confidence {
                kind: ConfidenceKind::FrontierEpistemic,
                score: 0.5,
                basis: "fixture".to_string(),
                method: ConfidenceMethod::ExpertJudgment,
                extraction_confidence: 1.0,
            },
            Provenance {
                source_type: "published_paper".to_string(),
                doi: Some("10.0000/vela-transaction-fixture".to_string()),
                url: None,
                title: "transaction fixture".to_string(),
                authors: Vec::new(),
                year: Some(2026),
                license: None,
                publisher: None,
                funders: Vec::new(),
                extraction: Extraction::default(),
                review: None,
                contributions: Vec::new(),
            },
            Flags::default(),
        )
    }

    #[test]
    fn malformed_campaign_does_not_block_existing_finding_briefing() {
        let temp = tempfile::tempdir().unwrap();
        let finding = finding();
        let target = finding.id.clone();
        let project =
            vela_protocol::project::assemble("finding-briefing", vec![finding], 0, 0, "fixture");
        vela_protocol::repo::init_repo(temp.path(), &project).unwrap();
        std::fs::write(temp.path().join("campaign.yaml"), "not: [valid").unwrap();
        let briefing = briefing_from_project(temp.path(), &target, &project, None, &[]).unwrap();
        assert_eq!(briefing.value["target"], target);
        assert!(briefing.value.get("task").is_none());
        assert!(briefing.target_task_binding.is_none());
    }

    #[test]
    fn historical_target_index_briefing_is_not_actionable() {
        let temp = tempfile::tempdir().unwrap();
        let project =
            vela_protocol::project::assemble("indexed-briefing", Vec::new(), 0, 0, "fixture");
        vela_protocol::repo::init_repo(temp.path(), &project).unwrap();
        std::fs::create_dir_all(temp.path().join("site/problems")).unwrap();
        let packet = br#"{"schema":"erdos-frontier.problem-work.v1","problem":1056,"statement":{"upstream_state":"open"},"residual_obligations":["one"]}"#;
        std::fs::write(temp.path().join("site/problems/1056.json"), packet).unwrap();
        let packet_digest = format!("sha256:{}", hex::encode(Sha256::digest(packet)));
        std::fs::write(
            temp.path().join("targets.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": "vela.target-index.v1",
                "frontier_id": project.frontier_id(),
                "as_of": {
                    "snapshot_hash": format!(
                        "sha256:{}",
                        vela_protocol::events::snapshot_hash(&project)
                    ),
                    "event_log_hash": format!(
                        "sha256:{}",
                        vela_protocol::events::event_log_hash(&project.events)
                    ),
                    "proposal_state_hash": format!("sha256:{}", "0".repeat(64)),
                },
                "targets": [{
                    "id": "erdos:1056",
                    "title": "Erdős 1056",
                    "why": "Nine banked attempts and open residual obligations",
                    "state": "open",
                    "rank": 0,
                    "objective": "Advance Erdős problem 1056 without repeating banked routes.",
                    "labels": ["banked", "erdos", "open"],
                    "packet": {
                        "path": "site/problems/1056.json",
                        "sha256": packet_digest,
                        "schema": "erdos-frontier.problem-work.v1",
                    },
                }],
            }))
            .unwrap(),
        )
        .unwrap();

        let error =
            briefing_from_project(temp.path(), "erdos:1056", &project, None, &[]).unwrap_err();
        assert!(error.contains("historical v1 inspection only"), "{error}");
    }

    fn signed_lease_candidate(
        original: &vela_protocol::project::Project,
        target: &str,
        actor: &str,
        key: &SigningKey,
        ttl_seconds: u64,
        prior_claim_event_id: Option<&str>,
        timestamp: &str,
    ) -> (vela_protocol::project::Project, Value) {
        let mut candidate = clone_project(original).unwrap();
        let pubkey = hex::encode(key.verifying_key().to_bytes());
        let mut payload = json!({
            "obligation_id": target,
            "lease_ttl_seconds": ttl_seconds,
            "claimant_actor": actor,
            "claimant_pubkey": pubkey,
        });
        if let Some(prior) = prior_claim_event_id {
            payload["prior_claim_event_id"] = json!(prior);
        }
        if ttl_seconds == 0 {
            payload["release_reason"] = json!("transaction fixture release");
        }
        let reason = if ttl_seconds == 0 {
            "transaction fixture release"
        } else {
            "transaction fixture claim"
        };
        let mut event =
            vela_protocol::events::new_finding_event(vela_protocol::events::FindingEventInput {
                kind: "attempt.claimed",
                finding_id: target,
                actor_id: actor,
                actor_type: "agent",
                reason,
                before_hash: "sha256:null",
                after_hash: "sha256:null",
                payload,
                caveats: Vec::new(),
                timestamp: Some(timestamp),
            });
        event.signature = Some(vela_protocol::sign::sign_event(&event, key).unwrap());
        vela_protocol::reducer::apply_event(&mut candidate, &event).unwrap();
        candidate.events.push(event.clone());
        let state_root_before = format!(
            "sha256:{}",
            vela_protocol::events::event_log_hash(&original.events)
        );
        let state_root_after = format!(
            "sha256:{}",
            vela_protocol::events::event_log_hash(&candidate.events)
        );
        let result = json!({
            "ok": true,
            "obligation": target,
            "claimed_by": actor,
            "ttl_seconds": ttl_seconds,
            "claim_event_id": event.id,
            "claimed_at": event.timestamp,
            "claimant_pubkey": pubkey,
            "prior_claim_event_id": prior_claim_event_id,
            "release_reason": if ttl_seconds == 0 { Some(reason) } else { None },
            "state_root_before": state_root_before,
            "state_root_after": state_root_after,
        });
        (candidate, result)
    }

    #[test]
    fn attempt_causal_root_allows_other_leases_but_rejects_nonlease_change() {
        let original = vela_protocol::project::assemble("lease-root", Vec::new(), 0, 0, "fixture");
        let key = SigningKey::from_bytes(&[0x76; 32]);
        let target = "seed:lease-root";
        let actor = "agent:lease-root";
        let (claimed, claim) = signed_lease_candidate(
            &original,
            target,
            actor,
            &key,
            86_400,
            None,
            "2026-07-14T10:00:00Z",
        );
        let mut session = attempt_size_fixture(0);
        session.target = target.to_string();
        session.actor = actor.to_string();
        session.frontier_id = original.frontier_id().to_string();
        session.base_event_log_root = claim["state_root_before"].as_str().unwrap().to_string();
        session.base_nonlease_event_log_root = nonlease_event_log_root(&original.events);
        session.lease.claim_event_id = claim["claim_event_id"].as_str().unwrap().to_string();
        session.lease.claimant_pubkey = claim["claimant_pubkey"].as_str().unwrap().to_string();

        let expected = claim["state_root_after"].as_str().unwrap();
        assert_eq!(
            attempt_causal_event_root(&claimed, &session).unwrap(),
            expected
        );
        let mut reordered = clone_project(&claimed).unwrap();
        reordered.events.reverse();
        assert_eq!(
            attempt_causal_event_root(&reordered, &session).unwrap(),
            expected,
            "the event-set commitment must not inherit storage order"
        );

        let (later, later_claim) = signed_lease_candidate(
            &claimed,
            "seed:later-event",
            "agent:later-event",
            &key,
            86_400,
            None,
            "2026-07-14T10:00:01Z",
        );
        assert_eq!(
            attempt_causal_event_root(&later, &session).unwrap(),
            later_claim["state_root_after"].as_str().unwrap(),
            "an unrelated coordination lease must not stale scientific work"
        );

        let mut changed = clone_project(&later).unwrap();
        let event =
            vela_protocol::events::new_finding_event(vela_protocol::events::FindingEventInput {
                kind: "finding.noted",
                finding_id: "seed:nonlease-change",
                actor_id: "agent:nonlease-change",
                actor_type: "agent",
                reason: "non-lease change fixture",
                before_hash: vela_protocol::events::NULL_HASH,
                after_hash: vela_protocol::events::NULL_HASH,
                payload: json!({"text": "a later scientific event changes the working base"}),
                caveats: Vec::new(),
                timestamp: Some("2026-07-14T10:00:02Z"),
            });
        changed.events.push(event);
        let error = attempt_causal_event_root(&changed, &session).unwrap_err();
        assert!(
            error.contains("non-lease changes"),
            "unexpected non-lease change error: {error}"
        );
    }

    #[test]
    fn lease_transaction_rejects_event_inserted_after_prepare_without_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let project = vela_protocol::project::assemble("lease-race", Vec::new(), 0, 0, "fixture");
        vela_protocol::repo::init_repo(temp.path(), &project).unwrap();
        let original = repo::load_from_path(temp.path()).unwrap();
        let key = SigningKey::from_bytes(&[0x71; 32]);
        let (candidate, result) = signed_lease_candidate(
            &original,
            "seed:planned",
            "agent:planned",
            &key,
            86_400,
            None,
            "2026-07-14T10:00:00Z",
        );
        let (winner, winner_result) = signed_lease_candidate(
            &original,
            "seed:winner",
            "agent:winner",
            &key,
            86_400,
            None,
            "2026-07-14T10:00:01Z",
        );
        let winner_event_id = winner_result["claim_event_id"]
            .as_str()
            .unwrap()
            .to_string();
        let planned_event_id = result["claim_event_id"].as_str().unwrap().to_string();
        let journal_dir = frontier_transaction_journal_dir(temp.path()).unwrap();
        let barrier = crate::frontier_txn::FrontierTxn::acquire_write_barrier_for_test(
            temp.path(),
            &journal_dir,
        )
        .unwrap();
        let error = transact_lease_candidate_with_barrier(
            temp.path(),
            barrier,
            &original,
            &candidate,
            result,
            || {
                // Model a non-cooperating filesystem writer between durable
                // prepare and the authoritative commit-marker comparison.
                repo::save_to_path(temp.path(), &winner)
            },
        )
        .unwrap_err();
        assert!(
            error.contains("event log"),
            "unexpected stale error: {error}"
        );
        let loaded = repo::load_from_path(temp.path()).unwrap();
        assert!(
            loaded
                .events
                .iter()
                .any(|event| event.id == winner_event_id)
        );
        assert!(
            !loaded
                .events
                .iter()
                .any(|event| event.id == planned_event_id)
        );
        assert!(
            loaded
                .attempt_claims
                .iter()
                .any(|claim| claim.obligation_id == "seed:winner")
        );
        assert!(
            !loaded
                .attempt_claims
                .iter()
                .any(|claim| claim.obligation_id == "seed:planned")
        );
    }
}
