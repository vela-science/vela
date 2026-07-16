//! The compounding loop's engine: claim → briefing → land → drop.
//! One implementation behind the CLI verbs (`work`, `land`) and the
//! MCP `work` tool, so agents and humans drive the same machinery.
//!
//! `land` is the loop's write edge and the home of the **Vela Receipt**
//! (`vela.receipt.v1`) — the portable JSON any external tool (Claude
//! Science exports, notebooks, Codex runs, foundry searches) hands
//! over to cross from activity into state:
//!
//! ```json
//! {
//!   "schema": "vela.receipt.v1",
//!   "claim": "what is now known / bounded / refuted",
//!   "type": "computational | theoretical | empirical | negative",
//!   "artifacts": [{"path": "…", "kind": "witness"}],
//!   "caveats": ["what this does NOT establish"],
//!   "verifier_runs": [{"method": "…", "outcome": "pass", "log": "…"}],
//!   "environment": {"…": "optional, carried into provenance"},
//!   "provenance": {"generated_by": "…", "co_author": "agent:…"}
//! }
//! ```
//!
//! Landing routes by the frontier's signed policy: **Permit** admits
//! canonically through the policy lane (no key ceremony — the human's
//! authority arrived earlier, once, as the policy signature); **Defer**
//! leaves the proposal pending, where it becomes a `vela sign` item;
//! **Deny** or a gate block refuses canonical admission. Landing is idempotent:
//! content addressing collapses byte-identical records, and an
//! already-applied proposal is the caller's exit 5.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_protocol::bundle::{ArtifactAvailability, ArtifactDisclosure, LocatorIntegrity};
use vela_protocol::proposals::policy_accept::{self, PolicyLaneRefusal};
use vela_protocol::receipt_v1::ReceiptV1;
use vela_protocol::repo;

/// Where a landing ended up.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum LandRoute {
    /// Admitted canonically under the signed policy (the autonomy lane).
    PolicyAdmitted { event_id: String, policy_id: String },
    /// Pending — a human's `vela sign` queue holds it now. Success-shaped.
    Deferred { reasons: Vec<String> },
    /// The same operation id and normalized request already crossed the
    /// durable boundary. A same-claim receipt with different evidence is never
    /// this route.
    ExactRetry { original_route: String },
}

#[derive(Debug)]
pub(crate) struct LandOutcome {
    pub operation_id: String,
    pub receipt_root: String,
    pub record_id: String,
    pub proposal_id: String,
    pub finding_id: String,
    /// Canonical event count observed under the frontier-wide recovery
    /// barrier before this landing was planned. New landings always populate
    /// this. A journal-free retry of older public state may report `None`
    /// because inventing a historical count would be misleading.
    pub accepted_event_count_before: Option<usize>,
    /// Canonical event count in the transaction postimage. Defer leaves this
    /// unchanged; a policy-admitted landing increments it by one.
    pub accepted_event_count_after: Option<usize>,
    pub route: LandRoute,
    pub publication: crate::config::git_publish::PublicationOutcome,
}

/// Transport-neutral landing result. CLI JSON adds only its command envelope;
/// MCP returns this projection inside its tool envelope. Scientific and
/// publication facts are therefore mapped exactly once.
#[derive(Debug, Serialize)]
pub(crate) struct LandOutcomeWire<'a> {
    pub operation_id: &'a str,
    pub receipt_root: &'a str,
    pub record_id: &'a str,
    pub proposal_id: &'a str,
    pub finding_id: &'a str,
    pub accepted_event_count_before: Option<usize>,
    pub accepted_event_count_after: Option<usize>,
    pub accepted_event_delta: Option<usize>,
    pub route: &'static str,
    /// Prior durable policy route for an exact retry; null for a first result.
    pub original_route: Option<&'a str>,
    pub detail: String,
    pub publication: &'a crate::config::git_publish::PublicationOutcome,
}

impl LandOutcome {
    pub(crate) fn accepted_event_delta(&self) -> Option<usize> {
        accepted_event_count_delta(
            self.accepted_event_count_before,
            self.accepted_event_count_after,
        )
        .expect("landing outcomes carry a validated monotone accepted-event count pair")
    }

    pub(crate) fn wire(&self) -> LandOutcomeWire<'_> {
        let (route, detail) = self.route.summary();
        LandOutcomeWire {
            operation_id: &self.operation_id,
            receipt_root: &self.receipt_root,
            record_id: &self.record_id,
            proposal_id: &self.proposal_id,
            finding_id: &self.finding_id,
            accepted_event_count_before: self.accepted_event_count_before,
            accepted_event_count_after: self.accepted_event_count_after,
            accepted_event_delta: self.accepted_event_delta(),
            route,
            original_route: self.route.original_route(),
            detail,
            publication: &self.publication,
        }
    }
}

fn accepted_event_count_delta(
    before: Option<usize>,
    after: Option<usize>,
) -> Result<Option<usize>, String> {
    match (before, after) {
        (None, None) => Ok(None),
        (Some(before), Some(after)) => after.checked_sub(before).map(Some).ok_or_else(|| {
            format!("accepted-event count decreased across landing ({before} -> {after})")
        }),
        _ => Err("accepted-event counts must either both be present or both be absent".to_string()),
    }
}

#[cfg(test)]
mod accepted_event_count_contract_tests {
    use super::{LandOutcome, LandRoute, accepted_event_count_delta};
    use crate::config::git_publish::{PublicationOutcome, PublicationState};

    #[test]
    fn accepted_event_delta_is_monotone_or_absent() {
        assert_eq!(accepted_event_count_delta(Some(12), Some(12)), Ok(Some(0)));
        assert_eq!(accepted_event_count_delta(Some(12), Some(13)), Ok(Some(1)));
        assert_eq!(accepted_event_count_delta(None, None), Ok(None));
        assert!(accepted_event_count_delta(Some(13), Some(12)).is_err());
        assert!(accepted_event_count_delta(Some(12), None).is_err());
        assert!(accepted_event_count_delta(None, Some(12)).is_err());
    }

    #[test]
    fn land_wire_projection_is_the_one_transport_neutral_contract() {
        let fixture = |route, after| LandOutcome {
            operation_id: format!("vop_{}", "1".repeat(64)),
            receipt_root: format!("sha256:{}", "2".repeat(64)),
            record_id: "vrc_wire".to_string(),
            proposal_id: "vsp_wire".to_string(),
            finding_id: "vf_wire".to_string(),
            accepted_event_count_before: Some(7),
            accepted_event_count_after: Some(after),
            route,
            publication: PublicationOutcome {
                state: PublicationState::Uncommitted {
                    candidate: None,
                    reason: "fixture".to_string(),
                },
                recovery_command: None,
            },
        };
        let cases = [
            (
                fixture(
                    LandRoute::PolicyAdmitted {
                        event_id: "vev_wire".to_string(),
                        policy_id: "vap_wire".to_string(),
                    },
                    8,
                ),
                "policy_admitted",
                None,
                "event vev_wire under vap_wire",
                1,
            ),
            (
                fixture(
                    LandRoute::Deferred {
                        reasons: vec!["human scientific judgment".to_string()],
                    },
                    7,
                ),
                "deferred",
                None,
                "human scientific judgment",
                0,
            ),
            (
                fixture(
                    LandRoute::ExactRetry {
                        original_route: "deferred".to_string(),
                    },
                    7,
                ),
                "exact_retry",
                Some("deferred"),
                "reused durable deferred result",
                0,
            ),
            (
                fixture(
                    LandRoute::ExactRetry {
                        original_route: "policy_admitted".to_string(),
                    },
                    7,
                ),
                "exact_retry",
                Some("policy_admitted"),
                "reused durable policy_admitted result",
                0,
            ),
        ];

        for (outcome, route, original_route, detail, delta) in cases {
            let wire = serde_json::to_value(outcome.wire()).unwrap();
            assert_eq!(wire["route"], route);
            assert_eq!(wire["original_route"], serde_json::json!(original_route));
            assert_eq!(wire["detail"], detail);
            assert_eq!(wire["accepted_event_delta"], delta);
            assert_eq!(wire.as_object().unwrap().len(), 12, "{wire}");
        }
    }
}

impl LandRoute {
    /// Machine-readable prior route. `detail` remains human-facing text.
    pub(crate) fn original_route(&self) -> Option<&str> {
        match self {
            LandRoute::ExactRetry { original_route } => Some(original_route),
            LandRoute::PolicyAdmitted { .. } | LandRoute::Deferred { .. } => None,
        }
    }

    /// The `(route, detail)` pair every landing surface reports — the CLI
    /// verb and the MCP `work` tool speak the same contract.
    pub(crate) fn summary(&self) -> (&'static str, String) {
        match self {
            LandRoute::PolicyAdmitted {
                event_id,
                policy_id,
            } => (
                "policy_admitted",
                format!("event {event_id} under {policy_id}"),
            ),
            LandRoute::Deferred { reasons } => ("deferred", reasons.join(", ")),
            LandRoute::ExactRetry { original_route } => (
                "exact_retry",
                format!("reused durable {original_route} result"),
            ),
        }
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
/// recoverable frontier-wide transaction used by proposal and landing writes.
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
    barrier: crate::frontier_txn::FrontierRecoveryBarrier,
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
    barrier: crate::frontier_txn::FrontierRecoveryBarrier,
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

pub(crate) fn transact_actor_registration<F>(frontier: &Path, build: F) -> Result<Value, String>
where
    F: FnOnce(
        &vela_protocol::project::Project,
    ) -> Result<(vela_protocol::project::Project, String, String), String>,
{
    let journal_dir = frontier_transaction_journal_dir(frontier)?;
    let barrier =
        crate::frontier_txn::FrontierTxn::acquire_recovery_barrier(frontier, &journal_dir)
            .map_err(|error| error.to_string())?;
    let original = repo::load_from_path(frontier)?;
    let state_root_before = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&original.events)
    );
    let (candidate, activation_event_id, activated_at) = build(&original)?;
    let state_root_after = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&candidate.events)
    );
    let result = json!({
        "ok": true,
        "command": "actor.activate",
        "activation_event_id": activation_event_id,
        "activated_at": activated_at,
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
            operation_namespace: "actor-registration-activation",
            request_schema: "vela.actor-registration-activation-request.internal.v1",
            request_event_id_field: "activation_event_id",
            result_event_id_field: "activation_event_id",
            result_timestamp_field: "activated_at",
            publication_summary: "activate actor registration",
            preserve_existing_event_bytes: true,
        },
        || Ok(()),
    )
}

/// The pre-loaded briefing for a target — the compounding payload the
/// session starts from. Problem-shaped targets get the full task packet;
/// rich campaign targets also carry their non-authorizing coordination task.
fn briefing_from_project(
    frontier: &Path,
    target: &str,
    project: &vela_protocol::project::Project,
) -> Result<Value, String> {
    let head = vela_protocol::events::event_log_hash(&project.events);
    let finding_target = project.findings.iter().any(|finding| finding.id == target);
    let packet = if finding_target {
        crate::server::tools::briefing_for_target(project, frontier, target)
    } else if let Some(packet) =
        vela_edge::frontier_next::target_index_packet_for_target(project, frontier, target)?
    {
        packet
    } else {
        crate::server::tools::briefing_for_target(project, frontier, target)
    };
    let task = if finding_target {
        None
    } else if let Some(task) =
        vela_edge::frontier_next::target_index_task_for_target(project, frontier, target)?
    {
        Some(task)
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
    Ok(offer)
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

const WORK_SESSION_SCHEMA: &str = "vela.work-session.internal.v1";
const TASK_CONTRACT_SCHEMA: &str = "vela.task-contract.internal.v1";
const WORK_SESSION_MAX_BYTES: usize = 2 * 1024 * 1024;

/// One private, ignored producer session. It is coordination and authoring
/// context, never canonical scientific state or an authority object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkSession {
    pub schema: String,
    pub session_id: String,
    pub target: String,
    pub frontier_id: String,
    pub base_event_log_root: String,
    pub base_nonlease_event_log_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_git_commit_oid: Option<String>,
    pub source_git_state: String,
    pub actor: String,
    pub created_at: String,
    pub lease: WorkSessionLease,
    pub task_contract: TaskContract,
    pub task_contract_root: String,
    pub receipt_builder: ReceiptBuilderSessionFacts,
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
pub(crate) struct ReceiptBuilderSessionFacts {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verifier_results: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedWorkSession {
    pub record: WorkSession,
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
            "Land one valid Receipt v1 whose evidence and caveats address this target."
                .to_string(),
        allowed_actions: vec![
            "inspect the pinned frontier and task briefing".to_string(),
            "run frozen verifiers and private search or experiment loops".to_string(),
            "create evidence artifacts and land a Receipt v1".to_string(),
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
            "run every verifier claimed by the receipt and report its actual outcome".to_string(),
            "state at least one caveat; if no material limitation is known, say so explicitly"
                .to_string(),
            "keep artifacts frontier-relative, bounded, and content-addressed at landing"
                .to_string(),
        ],
        escalation_path:
            "Land outside signed Permit scope as Defer; a key-custody human decides it in `vela sign`."
                .to_string(),
        authority_ceiling:
            "Producer evidence only. The session can create a receipt and proposal; it cannot create human acceptance."
                .to_string(),
    }
}

fn sha256_root(value: &impl Serialize) -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(value)?
    ))
}

fn nonlease_event_log_root(events: &[vela_protocol::events::StateEvent]) -> String {
    let nonlease = events
        .iter()
        .filter(|event| event.kind != vela_protocol::events::EVENT_KIND_ATTEMPT_CLAIMED)
        .cloned()
        .collect::<Vec<_>>();
    format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&nonlease)
    )
}

fn source_git_commit(frontier: &Path) -> (Option<String>, String) {
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(frontier).args([
        "--no-replace-objects",
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "fetch.fsckObjects=true",
        "rev-parse",
        "--verify",
        "HEAD^{commit}",
    ]);
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_REPLACE_REF_BASE",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_SYSTEM",
    ] {
        command.env_remove(name);
    }
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0");
    match command.output() {
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

fn encoded_work_session(session: &WorkSession) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(session)
        .map_err(|error| format!("encode work-session record: {error}"))?;
    bytes.push(b'\n');
    if bytes.len() > WORK_SESSION_MAX_BYTES {
        return Err(format!(
            "work-session record is {} bytes; limit is {WORK_SESSION_MAX_BYTES} bytes",
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
    source_git_commit_oid: Option<String>,
    source_git_state: &str,
    actor: &str,
    ttl_seconds: u64,
    task_contract: TaskContract,
    task_contract_root: String,
    briefing: Value,
) -> Result<(), String> {
    // These placeholders are at least as long as the generated identity and
    // timestamp fields. This rejects an impossible session before key lookup
    // or candidate signing; the exact record is measured again before commit.
    let timestamp_placeholder = "0".repeat(64);
    let session = WorkSession {
        schema: WORK_SESSION_SCHEMA.to_string(),
        session_id: format!("vws_{}", "0".repeat(64)),
        target: target.to_string(),
        frontier_id: frontier_id.to_string(),
        base_event_log_root: base_event_log_root.to_string(),
        base_nonlease_event_log_root: base_nonlease_event_log_root.to_string(),
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
        receipt_builder: ReceiptBuilderSessionFacts::default(),
        briefing,
    };
    encoded_work_session(&session).map(|_| ())
}

fn write_work_session(frontier: &Path, session: &WorkSession) -> Result<PathBuf, String> {
    use std::io::Write;

    let bytes = encoded_work_session(session)?;
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
                "work-session root must be a real directory: {}",
                work.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&work)
                .map_err(|error| format!("create work-session root {}: {error}", work.display()))?;
        }
        Err(error) => {
            return Err(format!(
                "inspect work-session root {}: {error}",
                work.display()
            ));
        }
    }
    let directory = session_dir(frontier, &session.target);
    match std::fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!(
                "work session must be a real directory: {}",
                directory.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&directory)
                .map_err(|error| format!("create work session {}: {error}", directory.display()))?;
        }
        Err(error) => {
            return Err(format!(
                "inspect work session {}: {error}",
                directory.display()
            ));
        }
    }
    let path = directory.join("session.json");
    if let Ok(metadata) = std::fs::symlink_metadata(&path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(format!(
            "work-session record must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    let temporary = directory.join(format!(
        ".session-{}-{}.tmp",
        std::process::id(),
        &session.session_id[4..20]
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
            .map_err(|error| format!("create work-session record: {error}"))?;
        file.write_all(&bytes)
            .map_err(|error| format!("write work-session record: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("persist work-session record: {error}"))?;
        std::fs::rename(&temporary, &path)
            .map_err(|error| format!("install work-session record: {error}"))?;
        Ok(path.clone())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn parse_work_session(path: &Path) -> Result<WorkSession, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("inspect work session {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "work-session record must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    if metadata.len() > WORK_SESSION_MAX_BYTES as u64 {
        return Err(format!(
            "work-session record is too large: {}",
            path.display()
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read work session {}: {error}", path.display()))?;
    let session: WorkSession = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "parse work session {}: {error}; remove this private stale session and rerun `vela work <target> --as <actor>`",
            path.display()
        )
    })?;
    if session.schema != WORK_SESSION_SCHEMA {
        return Err(format!(
            "unsupported work-session schema {} in {}",
            session.schema,
            path.display()
        ));
    }
    if session.task_contract.schema != TASK_CONTRACT_SCHEMA
        || sha256_root(&session.task_contract)? != session.task_contract_root
    {
        return Err(format!(
            "work-session task contract does not match its content root: {}",
            path.display()
        ));
    }
    Ok(session)
}

fn validate_active_session(
    frontier: &Path,
    actor: &str,
    session: &WorkSession,
) -> Result<(), String> {
    if session.actor != actor {
        return Err(format!(
            "work session {} belongs to {}, not {actor}",
            session.target, session.actor
        ));
    }
    let project = repo::load_from_path(frontier)?;
    if session.frontier_id != project.frontier_id() {
        return Err(format!(
            "work session {} belongs to a different frontier",
            session.target
        ));
    }
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
            "work session {} no longer owns the exact frontier lease",
            session.target
        ));
    }
    let expires = chrono::DateTime::parse_from_rfc3339(&current.claimed_at)
        .map_err(|error| format!("work lease timestamp: {error}"))?
        + chrono::Duration::seconds(current.lease_ttl_seconds as i64);
    if expires <= chrono::Utc::now() {
        return Err(format!("work session {} lease has expired", session.target));
    }
    Ok(())
}

/// Return the exact causal root for a session whose scientific base is still
/// current.
///
/// A work session deliberately pins `base_event_log_root` before its own
/// coordination event, so the task describes the scientific state the
/// producer actually started from. A policy-routed receipt must instead bind
/// the current set commitment including signed coordination leases. New
/// sessions separately pin the non-lease event set, so unrelated leases may
/// coexist while every scientific, provenance, or authority-event change
/// fails closed. The non-lease root is required because work sessions are
/// private scratch and have no compatibility lane.
fn work_session_landing_event_root(
    project: &vela_protocol::project::Project,
    session: &WorkSession,
) -> Result<String, String> {
    let matching_claims = project
        .events
        .iter()
        .filter(|event| event.id == session.lease.claim_event_id)
        .collect::<Vec<_>>();
    if matching_claims.len() != 1 {
        return Err("work session must resolve to exactly one frontier lease event".to_string());
    }
    let claim = matching_claims[0];
    if claim.kind != vela_protocol::events::EVENT_KIND_ATTEMPT_CLAIMED
        || claim.actor.id != session.actor
        || claim.payload.get("obligation_id").and_then(Value::as_str)
            != Some(session.target.as_str())
        || claim.payload.get("claimant_pubkey").and_then(Value::as_str)
            != Some(session.lease.claimant_pubkey.as_str())
    {
        return Err("work session lease event does not match its signed session facts".to_string());
    }
    if nonlease_event_log_root(&project.events) != session.base_nonlease_event_log_root {
        return Err(format!(
            "work session frontier has non-lease changes from its pinned state; remove the private session and rerun `vela work {} --as {}`",
            session.target, session.actor
        ));
    }
    Ok(format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    ))
}

/// Resolve an explicit target or infer the one active session owned by this
/// actor. Other actors' sessions never create ambiguity.
pub(crate) fn resolve_work_session(
    frontier: &Path,
    actor: &str,
    requested_target: Option<&str>,
) -> Result<ResolvedWorkSession, String> {
    if let Some(target) = requested_target {
        let directory = session_dir(frontier, target);
        let record = parse_work_session(&directory.join("session.json"))?;
        if record.target != target {
            return Err(format!(
                "work-session target {} does not match requested target {target}",
                record.target
            ));
        }
        validate_active_session(frontier, actor, &record)?;
        let relative_dir = directory
            .strip_prefix(frontier)
            .map_err(|_| "work session escaped the frontier".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        return Ok(ResolvedWorkSession {
            record,
            relative_dir,
        });
    }

    let root = frontier.join(".vela").join("work");
    let entries = std::fs::read_dir(&root)
        .map_err(|_| "no active work session; run `vela work <target>` first".to_string())?;
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("enumerate work sessions: {error}"))?;
        let path = entry.path().join("session.json");
        if !path.exists() {
            continue;
        }
        let record = parse_work_session(&path)?;
        if record.actor != actor {
            continue;
        }
        if validate_active_session(frontier, actor, &record).is_err() {
            continue;
        }
        let relative_dir = entry
            .path()
            .strip_prefix(frontier)
            .map_err(|_| "work session escaped the frontier".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        candidates.push(ResolvedWorkSession {
            record,
            relative_dir,
        });
    }
    candidates.sort_by(|left, right| left.record.target.cmp(&right.record.target));
    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err(format!(
            "no active work session for {actor}; run `vela work <target> --as {actor}` first"
        )),
        count => Err(format!(
            "{actor} has {count} active work sessions ({}); select one with `vela land --work <target>`",
            candidates
                .iter()
                .map(|session| session.record.target.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Claim/refresh a lease and install the single typed ignored session record.
/// CLI and MCP both call this function.
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
    if ttl_seconds == 0 {
        return Err("work lease TTL must be greater than zero".to_string());
    }
    let journal_dir = frontier_transaction_journal_dir(frontier)?;
    let barrier =
        crate::frontier_txn::FrontierTxn::acquire_recovery_barrier(frontier, &journal_dir)
            .map_err(|error| error.to_string())?;
    after_barrier()?;
    // Pin the producer's scientific base before the coordination lease adds
    // its own event. The claim remains the exact live-lease identity, while
    // the session and optional campaign task describe what state the producer
    // actually started from.
    let base_project = repo::load_from_path(frontier)?;
    let briefing = briefing_from_project(frontier, target, &base_project)?;
    let base_event_log_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&base_project.events)
    );
    let base_nonlease_event_log_root = nonlease_event_log_root(&base_project.events);
    let (source_git_commit_oid, source_git_state) = source_git_commit(frontier);
    let contract = task_contract(&briefing, target);
    let task_contract_root = sha256_root(&contract)?;
    preflight_work_session_size(
        target,
        &base_project.frontier_id(),
        &base_event_log_root,
        &base_nonlease_event_log_root,
        source_git_commit_oid.clone(),
        &source_git_state,
        actor,
        ttl_seconds,
        contract.clone(),
        task_contract_root.clone(),
        briefing.clone(),
    )?;
    let args = lease_args(frontier, target, actor, ttl_seconds, None, None);
    let mut candidate = clone_project(&base_project)?;
    let claim = vela_edge::vela_agent_mcp::apply_claim_task_to_project(&args, &mut candidate)?;
    if claim.get("ok").and_then(Value::as_bool) != Some(true) {
        let owner = claim
            .get("already_claimed_by")
            .and_then(Value::as_str)
            .unwrap_or("another actor");
        return Err(format!("work target {target} is already leased by {owner}"));
    }
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
    let expires_at = (chrono::DateTime::parse_from_rfc3339(&claimed_at)
        .map_err(|error| format!("lease timestamp: {error}"))?
        + chrono::Duration::seconds(ttl_seconds as i64))
    .to_rfc3339();
    let session_preimage = json!({
        "schema": WORK_SESSION_SCHEMA,
        "frontier_id": base_project.frontier_id(),
        "target": target,
        "actor": actor,
        "claim_event_id": claim_event_id,
        "task_contract_root": task_contract_root,
    });
    let session_id = format!(
        "vws_{}",
        vela_protocol::canonical::sha256_canonical(&session_preimage)?
    );
    let session = WorkSession {
        schema: WORK_SESSION_SCHEMA.to_string(),
        session_id,
        target: target.to_string(),
        frontier_id: base_project.frontier_id().to_string(),
        base_event_log_root,
        base_nonlease_event_log_root,
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
        receipt_builder: ReceiptBuilderSessionFacts::default(),
        briefing: briefing.clone(),
    };
    // Measure the exact record, including the signed event identity and
    // timestamp, before crossing the transaction commit marker.
    encoded_work_session(&session)?;
    let claim = transact_lease_candidate_with_barrier(
        frontier,
        barrier,
        &base_project,
        &candidate,
        claim,
        || Ok(()),
    )?;
    let path = write_work_session(frontier, &session)?;
    Ok(json!({
        "ok": true,
        "target": target,
        "claim": claim,
        "briefing": briefing,
        "session": session,
        "session_path": path.display().to_string(),
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
    let barrier =
        crate::frontier_txn::FrontierTxn::acquire_recovery_barrier(frontier, &journal_dir)
            .map_err(|error| error.to_string())?;
    let project = repo::load_from_path(frontier)?;
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
    let claimed_at = chrono::DateTime::parse_from_rfc3339(&lease.claimed_at)
        .map_err(|error| format!("work lease timestamp: {error}"))?;
    let expires_at = claimed_at + chrono::Duration::seconds(lease.lease_ttl_seconds as i64);
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
    let release = transact_lease_candidate_with_barrier(
        frontier,
        barrier,
        &project,
        &candidate,
        release,
        || Ok(()),
    )?;
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

/// Expand the small task-first flag surface into the same complete Receipt v1
/// accepted by file import. A unique active work session supplies the stable
/// emission context, so repeating the same normalized request produces the
/// same receipt and operation identity instead of consulting a new clock.
#[allow(clippy::too_many_arguments)]
pub(crate) fn author_receipt(
    frontier: &Path,
    actor: &str,
    requested_work: Option<&str>,
    claim: String,
    claim_type: String,
    replayability: String,
    artifact_flags: &[String],
    caveats: Vec<String>,
    predicted_observable: Option<String>,
    not_applicable: bool,
    performed_test: Option<String>,
    result: Option<String>,
    evidence: Vec<String>,
    counterevidence: Vec<String>,
) -> Result<ReceiptV1, String> {
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::receipt_v1::{
        ArtifactInput, ReceiptBuilder, ReceiptInput, ScientificChainAssertion,
    };

    if !(actor.starts_with("agent:") || actor.starts_with("ci:")) {
        return Err(
            "flag authoring requires an agent:/ci: producer identity; import a complete Receipt v1 for other producers"
                .to_string(),
        );
    }
    let work = resolve_work_session(frontier, actor, requested_work)?;
    let work_target = work.record.target.clone();
    let work_started_at = work.record.created_at.clone();
    let mut artifacts = Vec::new();
    let mut normalized_artifacts = Vec::new();
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
        let digest = hex::encode(Sha256::digest(&bytes));
        artifacts.push(
            ArtifactInput::new(
                path.to_string(),
                kind.to_string(),
                Some(digest.clone()),
                None,
            )
            .map_err(|error| error.to_string())?,
        );
        normalized_artifacts.push(json!({"path": path, "kind": kind, "sha256": digest}));
    }
    let project = repo::load_from_path(frontier)?;
    let event_root = work_session_landing_event_root(&project, &work.record)?;
    let policy_ref = vela_protocol::acceptance_policy::load_active_policy(frontier)?
        .map(|policy| policy.policy.id)
        .unwrap_or_else(|| "urn:vela:policy:none".to_string());
    let scientific_chain_requested = predicted_observable.is_some()
        || not_applicable
        || performed_test.is_some()
        || result.is_some()
        || !evidence.is_empty()
        || !counterevidence.is_empty();
    let scientific_chain = if scientific_chain_requested {
        let performed_test = performed_test
            .ok_or_else(|| "scientific-chain authoring requires --performed-test".to_string())?;
        let result =
            result.ok_or_else(|| "scientific-chain authoring requires --result".to_string())?;
        Some(
            ScientificChainAssertion::new(
                predicted_observable,
                not_applicable,
                performed_test,
                result,
                evidence,
                counterevidence,
            )
            .map_err(|error| error.to_string())?,
        )
    } else {
        None
    };
    let operation_preimage = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.land-authoring.internal.v1",
        "frontier_id": project.frontier_id(),
        "actor": actor,
        "work_target": work_target,
        "work_session_id": &work.record.session_id,
        "task_contract_root": &work.record.task_contract_root,
        "claim": claim,
        "claim_type": claim_type,
        "replayability": replayability,
        "artifacts": normalized_artifacts,
        "caveats": caveats,
        "scientific_chain": scientific_chain.as_ref().map(ScientificChainAssertion::as_value),
        "policy_ref": policy_ref,
    }))?;
    let operation_id = crate::operation_journal::operation_id("land", &operation_preimage);
    let key = vela_edge::vela_agent_mcp::agent_signing_key(Some(actor))?;
    let identity = IdentityBinding::build(
        IdentityBindingDraft {
            actor_id: actor.to_string(),
            actor_class: ActorClass::Agent,
            created_at: work_started_at.clone(),
        },
        &key,
    )?;
    let mut input = ReceiptInput::new(
        claim,
        claim_type,
        replayability,
        artifacts,
        caveats,
        Vec::new(),
        actor.to_string(),
        work_started_at,
        event_root,
        work.relative_dir,
        operation_id,
        policy_ref,
    )
    .and_then(|input| input.with_task_contract_root(work.record.task_contract_root.clone()))
    .map_err(|error| error.to_string())?;
    if let Some(scientific_chain) = scientific_chain {
        input = input
            .with_scientific_chain(scientific_chain)
            .map_err(|error| error.to_string())?;
    }
    ReceiptBuilder::build(input, &identity).map_err(|error| error.to_string())
}

/// Resolve the private coordination files that may be closed by this landing.
///
/// A receipt is allowed to retire a `session.json` only when it proves that it
/// came from that exact work session: the receipt producer, producer key,
/// operation id, task-contract root, pinned frontier, pinned event root, target,
/// and current
/// target lease must all agree. Receipts produced outside `.vela/work/` do not
/// touch coordination state. A malformed claim to an internal session fails
/// closed instead of silently deleting another producer's session.
fn active_work_session_close(
    frontier: &Path,
    project: &vela_protocol::project::Project,
    receipt: &ReceiptV1,
    executor: &str,
    operation_id: &str,
) -> Result<Option<crate::frontier_txn::RepoPath>, String> {
    use crate::frontier_txn::RepoPath;

    let Some(context) = receipt
        .as_value()
        .get("environment")
        .and_then(|value| value.get("vela:producer_context"))
        .and_then(Value::as_object)
    else {
        // Foreign Receipt v1 producers do not carry Vela's private work-session
        // context. They may land portable evidence, but can never close local
        // coordination state because there is no internal session claim to
        // validate.
        return Ok(None);
    };
    let base_path = context
        .get("base_path")
        .and_then(Value::as_str)
        .ok_or_else(|| "receipt producer context has no base_path".to_string())?;
    if !base_path.starts_with(".vela/work/") {
        return Ok(None);
    }

    let components = base_path.split('/').collect::<Vec<_>>();
    if components.len() != 3
        || components[0] != ".vela"
        || components[1] != "work"
        || components[2].is_empty()
    {
        return Err(format!(
            "receipt claims malformed internal work session {base_path}"
        ));
    }
    let session_name = components[2];
    let actor = context
        .get("actor")
        .and_then(Value::as_str)
        .ok_or_else(|| "receipt producer context has no actor".to_string())?;
    if actor != executor {
        return Err(format!(
            "work session producer {actor} does not match landing actor {executor}"
        ));
    }
    if context.get("operation_id").and_then(Value::as_str) != Some(operation_id) {
        return Err("work session receipt operation id does not match this landing".to_string());
    }
    let provenance_actor = receipt
        .as_value()
        .get("provenance")
        .and_then(|value| value.get("submitter"))
        .and_then(|value| value.get("actor"))
        .and_then(Value::as_str);
    if provenance_actor != Some(executor) {
        return Err("work session receipt provenance does not match its producer".to_string());
    }

    let session_path =
        RepoPath::parse(format!("{base_path}/session.json")).map_err(|error| error.to_string())?;
    let session_directory = frontier.join(base_path);
    let session_metadata = std::fs::symlink_metadata(&session_directory).map_err(|error| {
        format!(
            "inspect claimed work session {}: {error}",
            session_directory.display()
        )
    })?;
    if session_metadata.file_type().is_symlink() || !session_metadata.is_dir() {
        return Err(format!(
            "claimed work session must be a regular non-symlink directory: {}",
            session_directory.display()
        ));
    }
    let session = parse_work_session(&frontier.join(session_path.as_str()))?;
    let expected_directory = session_dir(frontier, &session.target);
    if expected_directory
        .file_name()
        .and_then(|name| name.to_str())
        != Some(session_name)
    {
        return Err(format!(
            "work-session target {} does not match its collision-safe directory",
            session.target
        ));
    }
    if session.actor != executor {
        return Err(format!(
            "work session producer {} does not match landing actor {executor}",
            session.actor
        ));
    }
    let frontier_id = project.frontier_id();
    if session.frontier_id != frontier_id {
        return Err("work session belongs to a different frontier".to_string());
    }
    let receipt_event_root = context
        .get("event_log_root")
        .and_then(Value::as_str)
        .ok_or_else(|| "work session receipt has no event log root".to_string())?;
    let landing_event_root = work_session_landing_event_root(project, &session)?;
    if receipt_event_root != landing_event_root {
        return Err("work session receipt is not bound through its exact lease event".to_string());
    }
    let receipt_task_root = context
        .get("task_contract_root")
        .and_then(Value::as_str)
        .ok_or_else(|| "work session receipt has no task-contract root".to_string())?;
    if receipt_task_root != session.task_contract_root {
        return Err("work session receipt is not bound to its task contract".to_string());
    }

    let lease = project
        .attempt_claims
        .iter()
        .find(|claim| claim.obligation_id == session.target)
        .ok_or_else(|| format!("work target {} has no frontier lease", session.target))?;
    if lease.claimant_actor != executor {
        return Err(format!(
            "work target {} is leased by {}, not {executor}",
            session.target, lease.claimant_actor
        ));
    }
    if lease.claim_event_id.as_deref() != Some(session.lease.claim_event_id.as_str()) {
        return Err("work session does not name the current lease event".to_string());
    }
    let producer_key = context
        .get("identity_binding")
        .and_then(|value| value.get("public_key_hex"))
        .and_then(Value::as_str)
        .ok_or_else(|| "work session receipt has no producer public key".to_string())?;
    if lease.claimant_pubkey != producer_key {
        return Err("work session receipt key does not match the target lease key".to_string());
    }

    Ok(Some(session_path))
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

/// Durable result stored in the private operation journal. Publication is a
/// later transport transaction and therefore is deliberately absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DurableLandResult {
    operation_id: String,
    receipt_root: String,
    record_id: String,
    proposal_id: String,
    finding_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    accepted_event_count_before: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    accepted_event_count_after: Option<usize>,
    route: LandRoute,
    /// Decision-critical facts bound into the private transaction plan. Permit
    /// also stamps its policy context and certificate into the accepted event;
    /// Defer retains these bytes only for exact crash recovery and rederives a
    /// fresh brief at human review time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    review_route: Option<StagedReviewRoute>,
}

fn validate_durable_event_counts(result: &DurableLandResult) -> Result<(), String> {
    accepted_event_count_delta(
        result.accepted_event_count_before,
        result.accepted_event_count_after,
    )
    .map(|_| ())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StagedReviewRoute {
    schema: String,
    policy_context: vela_protocol::acceptance_policy::PolicyContext,
    policy_decision: Option<vela_protocol::acceptance_policy::Decision>,
    policy_state: vela_protocol::proposals::policy_accept::PolicyState,
    permit_readiness: vela_protocol::proposals::policy_accept::PermitReadiness,
    reason_codes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    readiness_detail: Option<String>,
    engine_gate: vela_protocol::proposals::EngineVerdict,
}

/// Public, deterministic review input retained beside the proposal. It is not
/// a verdict or authority object; it records the exact staged facts that drove
/// routing so a clean clone does not need the producer's working directory or
/// private operation journal to reconstruct the review.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProposalReviewMaterial {
    schema: String,
    proposal_id: String,
    receipt_root: String,
    evaluated_at: String,
    route: StagedReviewRoute,
}

#[derive(Debug)]
struct PreparedArtifacts {
    records: Vec<vela_protocol::record::RecordArtifact>,
    writes: Vec<crate::frontier_txn::PlannedWrite>,
    read_set: Vec<crate::frontier_txn::InputBinding>,
}

/// Land one already-validated, lossless Receipt v1 through the single
/// recoverable frontier write edge. `push` changes only the post-commit Git
/// transport; it cannot change the scientific transaction or policy result.
pub(crate) fn land(
    frontier: &Path,
    receipt: &ReceiptV1,
    executor: &str,
    push: bool,
) -> Result<LandOutcome, String> {
    use crate::config::git_publish::{
        PublicationOutcome, PublicationState, PublishOptions, discover_exact_publication,
        discover_receipt_publication, exact_publication_preflight,
        exact_publication_resume_preflight, publication_disabled_reason, publication_is_busy,
        publication_repo_relative_path, publish_exact_delta,
    };
    use crate::frontier_txn::{
        ContentDigest, DeltaDraft, FrontierTxn, InputBinding, OperationId, PlannedWrite,
        RecoveryState, RepoPath, WriteClass,
    };

    let executor = executor.trim();
    if executor.is_empty() {
        return Err("land requires an explicit acting identity".to_string());
    }
    receipt
        .validate_safe_public_artifact_descriptors()
        .map_err(|error| error.to_string())?;
    // Read only the stable frontier identity before exact-retry lookup. New
    // semantic planning reloads the complete frontier while holding the
    // frontier-wide recovery barrier below.
    let observed = repo::load_from_path(frontier)?;
    let frontier_id = observed.frontier_id().to_string();
    let receipt_bytes = receipt
        .canonical_bytes()
        .map_err(|error| error.to_string())?;
    let receipt_root = receipt
        .canonical_root()
        .map_err(|error| error.to_string())?;
    let receipt_hex = receipt_root
        .strip_prefix("sha256:")
        .ok_or_else(|| "receipt root is not a canonical sha256 digest".to_string())?;
    let receipt_path = format!("records/receipts/sha256/{receipt_hex}.json");
    let request_bytes = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.land-request.internal.v1",
        "frontier_id": frontier_id,
        "executor": executor,
        "receipt_root": receipt_root,
    }))?;
    let request_root = ContentDigest::hash(&request_bytes);
    let operation_id = receipt_operation_id(receipt)?
        .map(OperationId::parse)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| OperationId::derive("land", request_root.as_str().as_bytes()));
    let journal_dir = frontier_transaction_journal_dir(frontier)?;
    let mut publish_opts = if push {
        PublishOptions::pushing()
    } else {
        PublishOptions::new(false)
    };
    let publication_disabled = publication_disabled_reason(frontier, &publish_opts);
    if publication_disabled.is_none() {
        publish_opts =
            publish_opts.with_preflight_inputs(receipt_publication_inputs(frontier, receipt)?);
    }

    // Exact retries are keyed by operation id AND normalized request root.
    // Reusing an operation id for different receipt bytes is an error, not a
    // claim-level deduplication shortcut.
    if let Some(existing) = FrontierTxn::open_if_present(frontier, &journal_dir, &operation_id)
        .map_err(|error| error.to_string())?
    {
        if existing.plan().request_root != request_root {
            return Err(format!(
                "operation {} is already bound to a different normalized receipt request",
                operation_id.as_str()
            ));
        }
        if matches!(existing.recovery_state(), RecoveryState::Aborted) {
            drop(existing);
        } else {
            let durable: DurableLandResult = serde_json::from_value(existing.plan().result.clone())
                .map_err(|error| format!("decode durable land result: {error}"))?;
            validate_durable_event_counts(&durable)?;
            let scientific_completed =
                matches!(existing.recovery_state(), RecoveryState::Completed);
            let public = existing
                .resolved_public_writes()
                .map_err(|error| error.to_string())?;
            let delta_root = existing.plan().canonical_delta.root().as_str().to_string();
            drop(existing);
            let publication_delta = if publication_disabled.is_some() {
                None
            } else {
                publication_delta(frontier, &delta_root, public)?
            };
            if scientific_completed && let Some(delta) = publication_delta.as_ref() {
                let anchor = publication_repo_relative_path(frontier, &receipt_path)?;
                if let Some(publication) =
                    discover_exact_publication(frontier, delta, &anchor, &publish_opts)?
                {
                    let original_route = durable.route.summary().0.to_string();
                    return Ok(outcome_from_durable(
                        durable,
                        LandRoute::ExactRetry { original_route },
                        publication,
                    ));
                }
            }
            let preflight = publication_delta.as_ref().map(|delta| {
                if scientific_completed {
                    exact_publication_resume_preflight(frontier, delta, &publish_opts)
                } else {
                    exact_publication_preflight(frontier, delta, &publish_opts)
                }
            });
            let preflight = preflight.transpose();
            let preflight = match preflight {
                Ok(value) => value,
                Err(outcome) if publication_is_busy(&outcome) => {
                    return Err(
                        "another Vela write/publication owns this repository; retry the same operation"
                            .to_string(),
                    );
                }
                Err(outcome) => {
                    if !scientific_completed {
                        let mut txn = FrontierTxn::open(frontier, &journal_dir, &operation_id)
                            .map_err(|error| error.to_string())?;
                        txn.mark_committed().map_err(|error| error.to_string())?;
                        txn.install().map_err(|error| error.to_string())?;
                        txn.complete().map_err(|error| error.to_string())?;
                    }
                    let original_route = durable.route.summary().0.to_string();
                    return Ok(outcome_from_durable(
                        durable,
                        LandRoute::ExactRetry { original_route },
                        outcome,
                    ));
                }
            };
            if !scientific_completed {
                let mut txn = FrontierTxn::open(frontier, &journal_dir, &operation_id)
                    .map_err(|error| error.to_string())?;
                txn.mark_committed().map_err(|error| error.to_string())?;
                txn.install().map_err(|error| error.to_string())?;
                txn.complete().map_err(|error| error.to_string())?;
            }
            let publication = match (publication_delta.as_ref(), preflight) {
                (Some(delta), Some(preflight)) => publish_exact_delta(
                    frontier,
                    "land",
                    &[durable.proposal_id.clone()],
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
                        reason: publication_disabled.clone().unwrap_or_else(|| {
                            "frontier transaction had no public Git delta".to_string()
                        }),
                    },
                    recovery_command: None,
                },
            };
            let original_route = durable.route.summary().0.to_string();
            return Ok(outcome_from_durable(
                durable,
                LandRoute::ExactRetry { original_route },
                publication,
            ));
        }
    }

    // A clone deliberately has no private FrontierTxn journals. Recover an
    // already-landed operation from its public, content-addressed proposal and
    // receipt links, then discover the unique Git commit that introduced both.
    // This path is provenance-only: it never recreates a scientific or Git
    // transaction and it still rejects a reused operation id with different
    // normalized producer identity or receipt bytes.
    if let Some(durable) = durable_land_result_from_public_state(
        &observed,
        frontier,
        &receipt_bytes,
        &receipt_root,
        operation_id.as_str(),
        executor,
    )? {
        validate_durable_event_counts(&durable)?;
        let publication = match discover_receipt_publication(
            frontier,
            &receipt_bytes,
            &receipt_root,
            operation_id.as_str(),
            &publish_opts,
        ) {
            Ok(Some(publication)) => publication,
            Ok(None) => PublicationOutcome {
                state: PublicationState::Uncommitted {
                    candidate: None,
                    reason: "durable public submission is not present in the selected Git history"
                        .to_string(),
                },
                recovery_command: None,
            },
            Err(error) if publication_disabled.is_some() => PublicationOutcome {
                state: PublicationState::Uncommitted {
                    candidate: None,
                    reason: publication_disabled
                        .clone()
                        .unwrap_or_else(|| format!("Git publication unavailable: {error}")),
                },
                recovery_command: None,
            },
            Err(error) => return Err(error),
        };
        let original_route = durable.route.summary().0.to_string();
        return Ok(outcome_from_durable(
            durable,
            LandRoute::ExactRetry { original_route },
            publication,
        ));
    }

    let recovery_barrier = FrontierTxn::acquire_recovery_barrier(frontier, &journal_dir)
        .map_err(|error| error.to_string())?;
    let policy_snapshot = vela_protocol::acceptance_policy::load_active_policy_snapshot(frontier)?;
    let original = repo::load_from_path(frontier)?;
    if original.frontier_id() != frontier_id {
        return Err("frontier identity changed while acquiring the write barrier".to_string());
    }
    let expected_event_hash = vela_protocol::events::event_log_hash(&original.events);
    let expected_event_root = format!("sha256:{expected_event_hash}");
    let fixed_time = chrono::Utc::now().to_rfc3339();
    let PreparedArtifacts {
        records: record_artifacts,
        writes: artifact_writes,
        mut read_set,
    } = prepare_receipt_artifacts(frontier, receipt)?;
    read_set.push(InputBinding {
        name: "receipt".to_string(),
        digest: ContentDigest::parse(receipt_root.clone()).map_err(|error| error.to_string())?,
    });
    read_set.extend(policy_authority_input_bindings(&policy_snapshot)?);
    let signing_key = if executor.starts_with("agent:") || executor.starts_with("ci:") {
        Some(vela_edge::vela_agent_mcp::agent_signing_key(Some(
            executor,
        ))?)
    } else {
        None
    };
    let record = crate::cli::records::build_record_for_land(
        receipt,
        &frontier_id,
        &expected_event_hash,
        &receipt_root,
        &receipt_path,
        operation_id.as_str(),
        executor,
        &fixed_time,
        record_artifacts,
        signing_key.as_ref(),
    )?;
    let record_path = format!("records/{}.json", record.id);
    let mut proposal = crate::cli::records::proposal_for_record_land(&record, &fixed_time)?;
    let claim = receipt
        .as_value()
        .get("claim")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let claim_type = receipt
        .as_value()
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut same_claim_findings = original
        .findings
        .iter()
        .filter(|finding| {
            !finding.flags.retracted
                && finding.assertion.text.trim() == claim.trim()
                && finding.assertion.assertion_type == claim_type
        })
        .map(|finding| finding.id.clone())
        .collect::<BTreeSet<_>>();
    // Deferred submissions live in proposals rather than `Project::findings`.
    // They still establish a prior scientific submission that an independent
    // same-claim receipt must relate to instead of disappearing through text
    // deduplication.
    for prior in &original.proposals {
        let Some(prior_finding) = prior.payload.get("finding") else {
            continue;
        };
        let assertion = prior_finding.get("assertion");
        if assertion
            .and_then(|value| value.get("text"))
            .and_then(Value::as_str)
            .is_some_and(|text| text.trim() == claim.trim())
            && assertion
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str)
                == Some(claim_type)
            && let Some(id) = prior_finding.get("id").and_then(Value::as_str)
        {
            same_claim_findings.insert(id.to_string());
        }
    }
    let same_claim_findings = same_claim_findings.into_iter().collect::<Vec<_>>();
    if !same_claim_findings.is_empty() {
        proposal
            .payload
            .get_mut("vela_submission")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "submission proposal lost its typed receipt links".to_string())?
            .insert(
                "same_claim_findings".to_string(),
                json!(same_claim_findings),
            );
        proposal.id = vela_protocol::proposals::proposal_id(&proposal);
    }
    let review_material_path = format!("records/review/sha256/{receipt_hex}.json");
    proposal
        .payload
        .get_mut("vela_submission")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "submission proposal lost its typed receipt links".to_string())?
        .insert(
            "review_material_path".to_string(),
            json!(&review_material_path),
        );
    if !proposal.source_refs.contains(&review_material_path) {
        proposal.source_refs.push(review_material_path.clone());
        proposal.source_refs.sort();
        proposal.source_refs.dedup();
    }
    proposal.id = vela_protocol::proposals::proposal_id(&proposal);
    let proposal_id = proposal.id.clone();
    let finding: vela_protocol::bundle::FindingBundle = serde_json::from_value(
        proposal
            .payload
            .get("finding")
            .cloned()
            .ok_or_else(|| "submission proposal has no finding".to_string())?,
    )
    .map_err(|error| format!("submission finding parse: {error}"))?;
    let finding_id = finding.id.clone();
    let mut candidate: vela_protocol::project::Project =
        serde_json::from_value(serde_json::to_value(&original).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    vela_protocol::proposals::insert_pending_in_frontier(&mut candidate, proposal.clone())?;
    let staged_policy_route = policy_accept::stage_policy_route_in_frontier_at(
        frontier,
        &candidate,
        &proposal_id,
        receipt,
        &fixed_time,
        &policy_snapshot,
    )
    .map_err(|error| error.to_string())?;
    let context = staged_policy_route.context().clone();
    let staged_review_route = StagedReviewRoute {
        schema: "vela.staged-review-route.internal.v2".to_string(),
        policy_context: context.clone(),
        policy_decision: staged_policy_route.decision().cloned(),
        policy_state: staged_policy_route.policy_state(),
        permit_readiness: staged_policy_route.permit_readiness(),
        reason_codes: staged_policy_route.policy_reason_codes().to_vec(),
        readiness_detail: staged_policy_route
            .readiness_detail()
            .map(ToString::to_string),
        engine_gate: staged_policy_route.engine_gate().clone(),
    };
    let mut snapshot_files = Vec::new();
    let route = if executor.starts_with("agent:") || executor.starts_with("ci:") {
        match policy_accept::apply_staged_policy_route_in_frontier(
            &mut candidate,
            staged_policy_route,
            executor,
        ) {
            Ok(outcome) => {
                snapshot_files = outcome.policy_snapshot_files;
                LandRoute::PolicyAdmitted {
                    event_id: outcome.event_id,
                    policy_id: outcome.certificate.policy_id,
                }
            }
            Err(PolicyLaneRefusal::Deferred { reasons }) => LandRoute::Deferred { reasons },
            Err(PolicyLaneRefusal::Denied { reasons }) => {
                return Err(format!(
                    "policy denies this landing; zero canonical and Git delta: {}",
                    reasons.join(", ")
                ));
            }
            Err(PolicyLaneRefusal::Error(error))
                if error.contains("engine gate blocked policy-lane") =>
            {
                LandRoute::Deferred {
                    reasons: vec![
                        "the signed policy permits this, but the engine gate requires human review"
                            .to_string(),
                    ],
                }
            }
            Err(PolicyLaneRefusal::Error(error)) => return Err(error),
        }
    } else {
        LandRoute::Deferred {
            reasons: vec!["human landing: decide it in `vela sign`".to_string()],
        }
    };

    let review_material = ProposalReviewMaterial {
        schema: "vela.proposal-review-material.internal.v2".to_string(),
        proposal_id: proposal_id.clone(),
        receipt_root: receipt_root.clone(),
        evaluated_at: fixed_time.clone(),
        route: staged_review_route.clone(),
    };
    let durable = DurableLandResult {
        operation_id: operation_id.as_str().to_string(),
        receipt_root: receipt_root.clone(),
        record_id: record.id.clone(),
        proposal_id: proposal_id.clone(),
        finding_id: finding_id.clone(),
        accepted_event_count_before: Some(original.events.len()),
        accepted_event_count_after: Some(candidate.events.len()),
        route: route.clone(),
        review_route: Some(staged_review_route),
    };
    validate_durable_event_counts(&durable)?;
    let work_session_close = active_work_session_close(
        frontier,
        &original,
        receipt,
        executor,
        operation_id.as_str(),
    )?;
    let mut managed = repo::render_vela_repo_files(frontier, &candidate)?;
    preserve_existing_event_bytes(frontier, &original, &candidate, &mut managed, "land")?;
    let mut writes =
        PlannedWrite::from_managed_files(managed).map_err(|error| error.to_string())?;
    writes.push(PlannedWrite::write(
        RepoPath::parse(&receipt_path).map_err(|error| error.to_string())?,
        WriteClass::PublicReview,
        receipt_bytes,
    ));
    writes.push(PlannedWrite::write(
        RepoPath::parse(&record_path).map_err(|error| error.to_string())?,
        WriteClass::PublicReview,
        pretty_json_bytes(&record)?,
    ));
    writes.push(PlannedWrite::write(
        RepoPath::parse(&review_material_path).map_err(|error| error.to_string())?,
        WriteClass::PublicReview,
        pretty_json_bytes(&review_material)?,
    ));
    if let Some(session_path) = work_session_close {
        writes.push(PlannedWrite::delete(
            session_path,
            WriteClass::PrivateCoordination,
        ));
    }
    writes.extend(artifact_writes);
    for snapshot in snapshot_files {
        let path = snapshot
            .relative_path
            .to_str()
            .ok_or_else(|| "policy snapshot path is not UTF-8".to_string())?;
        let target = frontier.join(&snapshot.relative_path);
        if let Ok(existing) = std::fs::read(&target)
            && existing != snapshot.bytes
        {
            return Err(format!(
                "content-addressed policy snapshot {} already exists with different bytes",
                target.display()
            ));
        }
        writes.push(PlannedWrite::write(
            RepoPath::parse(path).map_err(|error| error.to_string())?,
            WriteClass::Authority,
            snapshot.bytes,
        ));
    }
    let draft = DeltaDraft::prepare(frontier, writes).map_err(|error| error.to_string())?;
    let public = draft
        .resolved_public_writes()
        .map_err(|error| error.to_string())?;
    let publication_delta = if publication_disabled.is_some() {
        None
    } else {
        publication_delta(frontier, draft.delta.root().as_str(), public)?
    };
    let publication_preflight = publication_delta
        .as_ref()
        .map(|delta| exact_publication_preflight(frontier, delta, &publish_opts))
        .transpose();
    let publication_preflight = match publication_preflight {
        Ok(value) => value,
        Err(outcome) if publication_is_busy(&outcome) => {
            return Err(
                "another Vela write/publication owns this repository; scientific state was not changed"
                    .to_string(),
            );
        }
        Err(outcome) => {
            let plan = frontier_plan(
                frontier,
                &frontier_id,
                &fixed_time,
                operation_id.clone(),
                request_root,
                &expected_event_root,
                &candidate,
                &durable,
                read_set,
                &context,
                &draft,
            )?;
            let mut txn = FrontierTxn::prepare_with_barrier(recovery_barrier, plan, draft)
                .map_err(|error| error.to_string())?;
            txn.mark_committed().map_err(|error| error.to_string())?;
            txn.install().map_err(|error| error.to_string())?;
            txn.complete().map_err(|error| error.to_string())?;
            return Ok(outcome_from_durable(durable, route, outcome));
        }
    };
    let plan = frontier_plan(
        frontier,
        &frontier_id,
        &fixed_time,
        operation_id,
        request_root,
        &expected_event_root,
        &candidate,
        &durable,
        read_set,
        &context,
        &draft,
    )?;
    let mut txn = FrontierTxn::prepare_with_barrier(recovery_barrier, plan, draft)
        .map_err(|error| error.to_string())?;
    txn.mark_committed().map_err(|error| error.to_string())?;
    txn.install().map_err(|error| error.to_string())?;
    txn.complete().map_err(|error| error.to_string())?;
    drop(txn);
    let publication = match (publication_delta.as_ref(), publication_preflight) {
        (Some(delta), Some(preflight)) => publish_exact_delta(
            frontier,
            "land",
            &[proposal_id],
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
                    .unwrap_or_else(|| "frontier transaction had no public Git delta".to_string()),
            },
            recovery_command: None,
        },
    };
    Ok(outcome_from_durable(durable, route, publication))
}

#[allow(clippy::too_many_arguments)]
fn frontier_plan(
    frontier: &Path,
    frontier_id: &str,
    fixed_time: &str,
    operation_id: crate::frontier_txn::OperationId,
    request_root: crate::frontier_txn::ContentDigest,
    expected_event_root: &str,
    candidate: &vela_protocol::project::Project,
    durable: &DurableLandResult,
    mut read_set: Vec<crate::frontier_txn::InputBinding>,
    context: &vela_protocol::acceptance_policy::PolicyContext,
    draft: &crate::frontier_txn::DeltaDraft,
) -> Result<crate::frontier_txn::FrontierTxnPlan, String> {
    use crate::frontier_txn::{
        ContentDigest, FrontierBinding, FrontierTxnPlan, FrontierTxnPlanSpec, InputBinding,
        OperationKind,
    };
    let layout = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.frontier-layout.internal.v1",
        "frontier_id": frontier_id,
        "paths": draft
            .delta
            .writes()
            .iter()
            .map(|write| write.path.as_str())
            .collect::<Vec<_>>(),
    }))?;
    read_set.push(InputBinding {
        name: "policy_context".to_string(),
        digest: ContentDigest::parse(context.policy_language_digest()?)
            .map_err(|error| error.to_string())?,
    });
    read_set.sort_by(|left, right| left.name.cmp(&right.name));
    let expected_event_log_root =
        ContentDigest::parse(expected_event_root.to_string()).map_err(|error| error.to_string())?;
    let resulting_event_log_root = ContentDigest::parse(format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&candidate.events)
    ))
    .map_err(|error| error.to_string())?;
    let mut resulting_event_ids = candidate
        .events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    resulting_event_ids.sort();
    FrontierTxnPlan::new(
        FrontierTxnPlanSpec {
            kind: OperationKind::Submission,
            operation_id,
            request_root,
            frontier: FrontierBinding::new(frontier, frontier_id.to_string(), &layout)
                .map_err(|error| error.to_string())?,
            fixed_time: fixed_time.to_string(),
            expected_event_log_root,
            resulting_event_log_root,
            resulting_event_ids,
            read_set,
            result: serde_json::to_value(durable).map_err(|error| error.to_string())?,
        },
        draft.delta.clone(),
    )
    .map_err(|error| error.to_string())
}

fn prepare_receipt_artifacts(
    frontier: &Path,
    receipt: &ReceiptV1,
) -> Result<PreparedArtifacts, String> {
    use crate::frontier_txn::{ContentDigest, InputBinding, PlannedWrite, RepoPath, WriteClass};
    use vela_protocol::record::RecordArtifact;

    let values = receipt
        .as_value()
        .get("artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| "validated receipt is missing artifacts".to_string())?;
    let mut records = Vec::with_capacity(values.len());
    let mut public_blobs = BTreeMap::<String, Vec<u8>>::new();
    let mut read_set = Vec::new();
    let mut total_artifact_bytes = 0_u64;
    for (index, value) in values.iter().enumerate() {
        let path = value
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("artifact {index} has no path"))?;
        let kind = value
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("artifact {index} has no kind"))?;
        let disclosure = match value.get("disclosure").and_then(Value::as_str) {
            Some("restricted") => ArtifactDisclosure::Restricted,
            Some("public") | None => ArtifactDisclosure::Public,
            Some(other) => return Err(format!("artifact {index} has unknown disclosure {other}")),
        };
        let media_type = value
            .get("media_type")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let declared_size = value.get("size_bytes").and_then(Value::as_u64);
        let declared_hash = value.get("sha256").and_then(Value::as_str);
        if disclosure == ArtifactDisclosure::Restricted {
            if declared_hash.is_some() {
                return Err(format!(
                    "restricted artifact {index} must not expose a public equality digest"
                ));
            }
            if !(path.starts_with("custodian:") || path.starts_with("opaque:")) {
                return Err(format!(
                    "restricted artifact {index} needs a custodian: or opaque: locator"
                ));
            }
            records.push(RecordArtifact {
                kind: kind.to_string(),
                locator: path.to_string(),
                sha256: String::new(),
                size_bytes: None,
                media_type,
                disclosure,
                locator_integrity: parse_locator_integrity(value)?,
                availability: parse_availability(value)?,
                note: "restricted opaque reference; payload and opening are not in Git".to_string(),
            });
            continue;
        }

        let relative = Path::new(path);
        let local_candidate = !relative.is_absolute()
            && relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)));
        let local = if local_candidate {
            let candidate = frontier.join(relative);
            match std::fs::symlink_metadata(&candidate) {
                Ok(_) => Some(candidate),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => {
                    return Err(format!("inspect artifact {}: {error}", candidate.display()));
                }
            }
        } else {
            None
        };
        if local.is_some() {
            let label = format!("artifact {index}");
            let read_limit = public_artifact_read_limit(total_artifact_bytes, index)?;
            let bytes = crate::bounded_file::read_bounded_frontier_file(
                frontier, relative, read_limit, &label,
            )
            .map_err(|error| public_artifact_read_error(error, read_limit, index))?;
            account_public_artifact_bytes(&mut total_artifact_bytes, bytes.len() as u64, index)?;
            let digest = hex::encode(Sha256::digest(&bytes));
            if declared_hash.is_some_and(|declared| declared != digest) {
                return Err(format!("artifact {index} sha256 does not match its bytes"));
            }
            if declared_size.is_some_and(|declared| declared != bytes.len() as u64) {
                return Err(format!(
                    "artifact {index} size_bytes does not match its bytes"
                ));
            }
            let size_bytes = bytes.len() as u64;
            let blob_path = format!("records/artifacts/sha256/{digest}");
            public_blobs.entry(blob_path.clone()).or_insert(bytes);
            read_set.push(InputBinding {
                name: format!("artifact[{index}]"),
                digest: ContentDigest::parse(format!("sha256:{digest}"))
                    .map_err(|error| error.to_string())?,
            });
            records.push(RecordArtifact {
                kind: kind.to_string(),
                locator: blob_path,
                sha256: digest,
                size_bytes: Some(size_bytes),
                media_type: Some(
                    media_type.unwrap_or_else(|| "application/octet-stream".to_string()),
                ),
                disclosure,
                locator_integrity: LocatorIntegrity::Immutable,
                availability: ArtifactAvailability::Available,
                note: format!("copied from Receipt v1 artifact {index}"),
            });
            continue;
        }

        let uri = value
            .get("uri")
            .and_then(Value::as_str)
            .filter(|uri| !uri.trim().is_empty())
            .unwrap_or(path);
        let digest = declared_hash.ok_or_else(|| {
            format!(
                "public artifact {index} is not a frontier file and needs a sha256 + size_bytes + media_type descriptor"
            )
        })?;
        let size_bytes = declared_size.ok_or_else(|| {
            format!(
                "public artifact {index} is not a frontier file and needs an explicit size_bytes descriptor"
            )
        })?;
        let media_type = media_type.ok_or_else(|| {
            format!(
                "public artifact {index} is not a frontier file and needs an explicit media_type descriptor"
            )
        })?;
        records.push(RecordArtifact {
            kind: kind.to_string(),
            locator: uri.to_string(),
            sha256: digest.to_string(),
            size_bytes: Some(size_bytes),
            media_type: Some(media_type),
            disclosure,
            locator_integrity: parse_locator_integrity(value)?,
            availability: parse_availability(value)?,
            note: "public content descriptor; locator was not dereferenced".to_string(),
        });
        read_set.push(InputBinding {
            name: format!("artifact[{index}]"),
            digest: ContentDigest::parse(format!("sha256:{digest}"))
                .map_err(|error| error.to_string())?,
        });
    }
    let writes = public_blobs
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
    Ok(PreparedArtifacts {
        records,
        writes,
        read_set,
    })
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
    use super::{account_public_artifact_bytes, public_artifact_read_limit};
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
}

/// Return only safe, currently materialized public receipt inputs. Exact Git
/// publication binds these worktree bytes and permits them as read-only dirt;
/// the candidate still contains only the transaction's content-addressed copy.
/// Missing or no-longer-regular paths are omitted here so recovery never needs
/// mutable producer files; initial semantic preparation reports those errors.
fn receipt_publication_inputs(
    frontier: &Path,
    receipt: &ReceiptV1,
) -> Result<Vec<PathBuf>, String> {
    let canonical_frontier = frontier
        .canonicalize()
        .map_err(|error| format!("canonicalize frontier: {error}"))?;
    let mut inputs = receipt
        .as_value()
        .get("artifacts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|artifact| artifact.get("disclosure").and_then(Value::as_str) != Some("restricted"))
        .filter_map(|artifact| artifact.get("path").and_then(Value::as_str))
        .filter_map(|path| {
            let relative = PathBuf::from(path);
            (!relative.is_absolute()
                && relative
                    .components()
                    .all(|component| matches!(component, std::path::Component::Normal(_))))
            .then_some(relative)
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

/// Bind the exact active-policy bytes (or their absence) into the scientific
/// transaction's marker-time read set. The frontier lock coordinates Vela
/// writers; these bindings also fail closed against a manual policy rotation
/// by a process that ignores the lock.
fn policy_authority_input_bindings(
    snapshot: &vela_protocol::acceptance_policy::ActivePolicySnapshot,
) -> Result<Vec<crate::frontier_txn::InputBinding>, String> {
    use crate::frontier_txn::{InputBinding, RepoPath};

    [
        (
            ".vela/policies/active.json",
            snapshot.policy_bytes.as_deref(),
        ),
        (
            ".vela/policies/active.sig.json",
            snapshot.signature_bytes.as_deref(),
        ),
    ]
    .into_iter()
    .map(|(relative, bytes)| {
        let path = RepoPath::parse(relative).map_err(|error| error.to_string())?;
        InputBinding::file_snapshot(path, bytes).map_err(|error| error.to_string())
    })
    .collect()
}

fn parse_locator_integrity(value: &Value) -> Result<LocatorIntegrity, String> {
    match value.get("locator_integrity").and_then(Value::as_str) {
        Some("immutable") => Ok(LocatorIntegrity::Immutable),
        Some("mutable") => Ok(LocatorIntegrity::Mutable),
        Some("unknown") | None => Ok(LocatorIntegrity::Unknown),
        Some(other) => Err(format!("unknown locator_integrity {other}")),
    }
}

fn parse_availability(value: &Value) -> Result<ArtifactAvailability, String> {
    match value.get("availability").and_then(Value::as_str) {
        Some("available") => Ok(ArtifactAvailability::Available),
        Some("unavailable") => Ok(ArtifactAvailability::Unavailable),
        Some("unknown") | None => Ok(ArtifactAvailability::Unknown),
        Some(other) => Err(format!("unknown artifact availability {other}")),
    }
}

fn receipt_operation_id(receipt: &ReceiptV1) -> Result<Option<String>, String> {
    let environment = receipt.as_value().get("environment");
    let from_environment = environment
        .and_then(|value| value.get("vela:producer_context"))
        .and_then(|value| value.get("operation_id"))
        .and_then(Value::as_str);
    let from_provenance = receipt
        .as_value()
        .get("provenance")
        .and_then(|value| value.get("submitter"))
        .and_then(|value| value.get("operation_id"))
        .and_then(Value::as_str);
    if let (Some(left), Some(right)) = (from_environment, from_provenance)
        && left != right
    {
        return Err("receipt carries conflicting producer operation ids".to_string());
    }
    Ok(from_environment
        .or(from_provenance)
        .map(ToString::to_string))
}

fn pretty_json_bytes(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
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

fn durable_land_result_from_public_state(
    project: &vela_protocol::project::Project,
    frontier: &Path,
    expected_receipt_bytes: &[u8],
    expected_receipt_root: &str,
    operation_id: &str,
    executor: &str,
) -> Result<Option<DurableLandResult>, String> {
    let mut matches = project.proposals.iter().filter(|proposal| {
        proposal
            .payload
            .get("vela_submission")
            .and_then(|submission| submission.get("operation_id"))
            .and_then(Value::as_str)
            == Some(operation_id)
    });
    let Some(proposal) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err(format!(
            "public frontier contains multiple proposals for operation {operation_id}"
        ));
    }
    let submission = proposal
        .payload
        .get("vela_submission")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "proposal {} lost its typed public submission links",
                proposal.id
            )
        })?;
    let receipt_root = submission
        .get("receipt_root")
        .and_then(Value::as_str)
        .ok_or_else(|| "public submission has no receipt_root".to_string())?;
    if receipt_root != expected_receipt_root {
        return Err(format!(
            "operation {operation_id} is already bound to a different public receipt root"
        ));
    }
    if proposal.actor.id != executor {
        return Err(format!(
            "operation {operation_id} was produced by {}, not {executor}",
            proposal.actor.id
        ));
    }
    let receipt_path = submission
        .get("receipt_path")
        .and_then(Value::as_str)
        .ok_or_else(|| "public submission has no receipt_path".to_string())?;
    let expected_hex = expected_receipt_root
        .strip_prefix("sha256:")
        .ok_or_else(|| "public submission receipt root is not sha256".to_string())?;
    let expected_path = format!("records/receipts/sha256/{expected_hex}.json");
    if receipt_path != expected_path {
        return Err(format!(
            "public submission receipt path {receipt_path} does not match its content root"
        ));
    }
    let stored_bytes = crate::bounded_file::read_bounded_frontier_file(
        frontier,
        Path::new(receipt_path),
        crate::bounded_file::RECEIPT_MAX_BYTES,
        "durable public receipt",
    )
    .map_err(|error| error.to_string())?;
    let stored = ReceiptV1::parse(&stored_bytes).map_err(|error| error.to_string())?;
    if stored.canonical_root().map_err(|error| error.to_string())? != expected_receipt_root
        || stored
            .canonical_bytes()
            .map_err(|error| error.to_string())?
            != expected_receipt_bytes
    {
        return Err(format!(
            "durable public receipt {receipt_path} differs from the exact retry input"
        ));
    }

    let record_id = submission
        .get("record_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "public submission has no record_id".to_string())?
        .to_string();
    let finding_id = proposal
        .payload
        .get("finding")
        .and_then(|finding| finding.get("id"))
        .and_then(Value::as_str)
        .unwrap_or(&proposal.target.id)
        .to_string();
    let policy_lane_errors = policy_accept::verify_policy_lane_events(project, frontier);
    if !policy_lane_errors.is_empty() {
        return Err(format!(
            "durable public retry refused an unverifiable policy lane: {}",
            policy_lane_errors.join(" | ")
        ));
    }
    let route = proposal
        .applied_event_id
        .as_deref()
        .and_then(|event_id| project.events.iter().find(|event| event.id == event_id))
        .and_then(|event| {
            let lane = event.payload.get(policy_accept::POLICY_LANE_PAYLOAD_KEY)?;
            let policy_id = lane.get("policy_id")?.as_str()?;
            Some(LandRoute::PolicyAdmitted {
                event_id: event.id.clone(),
                policy_id: policy_id.to_string(),
            })
        })
        .unwrap_or_else(|| LandRoute::Deferred {
            // The exact public route facts live in the review-material blob.
            // LandOutcome's retry surface exposes only the route class, so do
            // not invent or duplicate its explanatory reasons here.
            reasons: Vec::new(),
        });
    Ok(Some(DurableLandResult {
        operation_id: operation_id.to_string(),
        receipt_root: expected_receipt_root.to_string(),
        record_id,
        proposal_id: proposal.id.clone(),
        finding_id,
        // Public state proves the route but does not retain the historical
        // before/after counts for pre-existing submissions. Returning None is
        // preferable to deriving a false count from a later frontier head.
        accepted_event_count_before: None,
        accepted_event_count_after: None,
        route,
        review_route: None,
    }))
}

fn outcome_from_durable(
    durable: DurableLandResult,
    route: LandRoute,
    publication: crate::config::git_publish::PublicationOutcome,
) -> LandOutcome {
    LandOutcome {
        operation_id: durable.operation_id,
        receipt_root: durable.receipt_root,
        record_id: durable.record_id,
        proposal_id: durable.proposal_id,
        finding_id: durable.finding_id,
        accepted_event_count_before: durable.accepted_event_count_before,
        accepted_event_count_after: durable.accepted_event_count_after,
        route,
        publication,
    }
}

#[cfg(test)]
mod workflow_transaction_tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use vela_protocol::bundle::{
        Assertion, Conditions, Confidence, ConfidenceKind, ConfidenceMethod, Evidence, Extraction,
        FindingBundle, Flags, Provenance,
    };

    fn work_session_size_fixture(padding: usize) -> WorkSession {
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
        WorkSession {
            schema: WORK_SESSION_SCHEMA.to_string(),
            session_id: format!("vws_{}", "0".repeat(64)),
            target: "seed:size-fixture".to_string(),
            frontier_id: "vfr_size_fixture".to_string(),
            base_event_log_root: format!("sha256:{}", "0".repeat(64)),
            base_nonlease_event_log_root: format!("sha256:{}", "0".repeat(64)),
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
            receipt_builder: ReceiptBuilderSessionFacts::default(),
            briefing: json!({"padding": "x".repeat(padding)}),
        }
    }

    const PRODUCER_AUTHORITY_CEILING_FOR_TEST: &str =
        "Producer evidence only; fixture cannot accept truth.";

    #[test]
    fn work_session_requires_the_nonlease_root_without_a_legacy_fallback() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("session.json");
        let mut value = serde_json::to_value(work_session_size_fixture(0)).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("base_nonlease_event_log_root");
        std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let error = parse_work_session(&path).unwrap_err();
        assert!(error.contains("base_nonlease_event_log_root"), "{error}");
        assert!(error.contains("rerun `vela work"), "{error}");
    }

    #[test]
    fn work_session_size_ceiling_is_exact_and_preflight_precedes_claim() {
        let empty = work_session_size_fixture(0);
        let empty_len = serde_json::to_vec_pretty(&empty).unwrap().len() + 1;
        assert!(empty_len < WORK_SESSION_MAX_BYTES);
        let at_limit = work_session_size_fixture(WORK_SESSION_MAX_BYTES - empty_len);
        assert_eq!(
            encoded_work_session(&at_limit).unwrap().len(),
            WORK_SESSION_MAX_BYTES
        );
        let over_limit = work_session_size_fixture(WORK_SESSION_MAX_BYTES - empty_len + 1);
        assert!(
            encoded_work_session(&over_limit)
                .unwrap_err()
                .contains("limit")
        );

        let temp = tempfile::tempdir().unwrap();
        let project =
            vela_protocol::project::assemble("preflight-no-lease", Vec::new(), 0, 0, "fixture");
        vela_protocol::repo::init_repo(temp.path(), &project).unwrap();
        let before = repo::load_from_path(temp.path()).unwrap();
        let oversized_actor = format!("agent:{}", "x".repeat(WORK_SESSION_MAX_BYTES));
        let error = open_session(
            temp.path(),
            "seed:oversized-session",
            &oversized_actor,
            86_400,
        )
        .unwrap_err();
        assert!(error.contains("work-session record is"), "{error}");
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
        let briefing = briefing_from_project(temp.path(), &target, &project).unwrap();
        assert_eq!(briefing["target"], target);
        assert!(briefing.get("task").is_none());
    }

    #[test]
    fn target_index_briefing_loads_the_hash_pinned_packet_and_objective() {
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
                    "labels": ["erdos", "open", "banked"],
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

        let briefing = briefing_from_project(temp.path(), "erdos:1056", &project).unwrap();
        assert_eq!(briefing["briefing"]["packet"]["problem"], 1056);
        assert_eq!(briefing["task"]["kind"], "target_packet");
        assert_eq!(
            task_contract(&briefing, "erdos:1056").objective,
            "Advance Erdős problem 1056 without repeating banked routes."
        );
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
    fn work_session_landing_root_allows_other_leases_but_rejects_nonlease_change() {
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
        let mut session = work_session_size_fixture(0);
        session.target = target.to_string();
        session.actor = actor.to_string();
        session.frontier_id = original.frontier_id().to_string();
        session.base_event_log_root = claim["state_root_before"].as_str().unwrap().to_string();
        session.base_nonlease_event_log_root = nonlease_event_log_root(&original.events);
        session.lease.claim_event_id = claim["claim_event_id"].as_str().unwrap().to_string();
        session.lease.claimant_pubkey = claim["claimant_pubkey"].as_str().unwrap().to_string();

        let expected = claim["state_root_after"].as_str().unwrap();
        assert_eq!(
            work_session_landing_event_root(&claimed, &session).unwrap(),
            expected
        );
        let mut reordered = clone_project(&claimed).unwrap();
        reordered.events.reverse();
        assert_eq!(
            work_session_landing_event_root(&reordered, &session).unwrap(),
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
            work_session_landing_event_root(&later, &session).unwrap(),
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
        let error = work_session_landing_event_root(&changed, &session).unwrap_err();
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
        let barrier =
            crate::frontier_txn::FrontierTxn::acquire_recovery_barrier(temp.path(), &journal_dir)
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
