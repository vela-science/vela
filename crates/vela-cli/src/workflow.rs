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
    pub route: LandRoute,
    pub publication: crate::config::git_publish::PublicationOutcome,
}

impl LandRoute {
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

/// Claim a lease on a target (the same engine the MCP work tool uses).
pub(crate) fn claim(
    frontier: &Path,
    target: &str,
    actor: &str,
    ttl_seconds: Option<u64>,
) -> Result<Value, String> {
    let args = json!({
        "frontier_path": frontier.display().to_string(),
        "obligation_id": target,
        "agent_actor": actor,
        "ttl_seconds": ttl_seconds,
    });
    let raw = vela_edge::vela_agent_mcp::claim_task(&args)?;
    serde_json::from_str(&raw).map_err(|e| format!("claim response: {e}"))
}

/// The pre-loaded briefing for a target — the compounding payload the
/// session starts from. Problem-shaped targets get the full task
/// packet; everything else gets the frontier-level slice.
pub(crate) fn briefing(frontier: &Path, target: &str) -> Result<Value, String> {
    let project = repo::load_from_path(frontier)?;
    let head = vela_protocol::events::event_log_hash(&project.events);
    let packet = crate::server::tools::briefing_for_target(&project, frontier, target);
    Ok(json!({
        "schema": "vela.next_offer.v0.1",
        "target": target,
        "pinned_state": {
            "frontier_id": project.frontier_id().to_string(),
            "event_log_hash": head,
        },
        "briefing": packet,
    }))
}

/// The session directory for a target within a frontier.
pub(crate) fn session_dir(frontier: &Path, target: &str) -> PathBuf {
    let safe: String = target
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    frontier.join(".vela").join("work").join(safe)
}

/// Expand the small task-first flag surface into the same complete Receipt v1
/// accepted by file import. A unique active work session supplies the stable
/// emission context, so repeating the same normalized request produces the
/// same receipt and operation identity instead of consulting a new clock.
pub(crate) fn author_receipt(
    frontier: &Path,
    actor: &str,
    claim: String,
    claim_type: String,
    replayability: String,
    artifact_flags: &[String],
    caveats: Vec<String>,
) -> Result<ReceiptV1, String> {
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::receipt_v1::{ArtifactInput, ReceiptBuilder, ReceiptInput};

    if !(actor.starts_with("agent:") || actor.starts_with("ci:")) {
        return Err(
            "flag authoring requires an agent:/ci: producer identity; import a complete Receipt v1 for other producers"
                .to_string(),
        );
    }
    let (work_target, work_started_at) = unique_work_context(frontier)?;
    let canonical_frontier = frontier
        .canonicalize()
        .map_err(|error| format!("canonicalize frontier: {error}"))?;
    let mut artifacts = Vec::new();
    let mut normalized_artifacts = Vec::new();
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
        let candidate = canonical_frontier.join(relative);
        let metadata = std::fs::symlink_metadata(&candidate)
            .map_err(|error| format!("artifact {}: {error}", candidate.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "artifact {} must be a regular non-symlink file",
                candidate.display()
            ));
        }
        let canonical = candidate
            .canonicalize()
            .map_err(|error| format!("canonicalize artifact {}: {error}", candidate.display()))?;
        if !canonical.starts_with(&canonical_frontier) {
            return Err(format!(
                "artifact {} escapes the frontier",
                candidate.display()
            ));
        }
        let bytes = std::fs::read(&canonical)
            .map_err(|error| format!("read artifact {}: {error}", canonical.display()))?;
        let digest = hex::encode(Sha256::digest(bytes));
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
    let event_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    );
    let policy_ref = vela_protocol::acceptance_policy::load_active_policy(frontier)?
        .map(|policy| policy.policy.id)
        .unwrap_or_else(|| "urn:vela:policy:none".to_string());
    let operation_preimage = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.land-authoring.internal.v1",
        "frontier_id": project.frontier_id(),
        "actor": actor,
        "work_target": work_target,
        "claim": claim,
        "claim_type": claim_type,
        "replayability": replayability,
        "artifacts": normalized_artifacts,
        "caveats": caveats,
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
    let input = ReceiptInput::new(
        claim,
        claim_type,
        replayability,
        artifacts,
        caveats,
        Vec::new(),
        actor.to_string(),
        work_started_at,
        event_root,
        format!(".vela/work/{work_target}"),
        operation_id,
        policy_ref,
    )
    .map_err(|error| error.to_string())?;
    ReceiptBuilder::build(input, &identity).map_err(|error| error.to_string())
}

fn unique_work_context(frontier: &Path) -> Result<(String, String), String> {
    let root = frontier.join(".vela").join("work");
    let entries = std::fs::read_dir(&root)
        .map_err(|_| "no active work session; run `vela work <target>` first".to_string())?;
    let mut offers = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let offer = entry.path().join("offer.json");
            offer.is_file().then_some((entry.file_name(), offer))
        })
        .collect::<Vec<_>>();
    offers.sort_by(|left, right| left.0.cmp(&right.0));
    if offers.len() != 1 {
        return Err(format!(
            "flag authoring needs exactly one active work session, found {}; run `vela work <target>` or use a Receipt v1 file",
            offers.len()
        ));
    }
    let (target, offer) = offers.remove(0);
    let target = target
        .to_str()
        .ok_or_else(|| "active work target is not UTF-8".to_string())?
        .to_string();
    let started = std::fs::metadata(&offer)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| format!("inspect {}: {error}", offer.display()))?;
    let started = chrono::DateTime::<chrono::Utc>::from(started).to_rfc3339();
    Ok((target, started))
}

/// Resolve the private coordination files that may be closed by this landing.
///
/// A receipt is allowed to retire an `offer.json` only when it proves that it
/// came from that exact work session: the receipt producer, producer key,
/// operation id, pinned frontier, pinned event root, offer target, and current
/// target lease must all agree. Receipts produced outside `.vela/work/` do not
/// touch coordination state. A malformed claim to an internal session fails
/// closed instead of silently deleting another producer's offer.
fn active_work_session_close(
    frontier: &Path,
    project: &vela_protocol::project::Project,
    receipt: &ReceiptV1,
    executor: &str,
    operation_id: &str,
) -> Result<
    Option<(
        crate::frontier_txn::RepoPath,
        crate::frontier_txn::RepoPath,
        String,
    )>,
    String,
> {
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
    let session = components[2];
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

    let offer_path =
        RepoPath::parse(format!("{base_path}/offer.json")).map_err(|error| error.to_string())?;
    let completed_path =
        RepoPath::parse(format!("{base_path}/landed.json")).map_err(|error| error.to_string())?;
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
    let offer_file = frontier.join(offer_path.as_str());
    let offer_metadata = std::fs::symlink_metadata(&offer_file)
        .map_err(|error| format!("inspect work offer {}: {error}", offer_file.display()))?;
    if offer_metadata.file_type().is_symlink() || !offer_metadata.is_file() {
        return Err(format!(
            "work offer must be a regular non-symlink file: {}",
            offer_file.display()
        ));
    }
    if let Ok(metadata) = std::fs::symlink_metadata(frontier.join(completed_path.as_str())) {
        return Err(if metadata.file_type().is_symlink() {
            "work session completion marker must not be a symlink".to_string()
        } else {
            "work session already has a completion marker".to_string()
        });
    }

    let offer_bytes = std::fs::read(&offer_file)
        .map_err(|error| format!("read work offer {}: {error}", offer_file.display()))?;
    let offer: Value = serde_json::from_slice(&offer_bytes)
        .map_err(|error| format!("parse work offer {}: {error}", offer_file.display()))?;
    if offer.get("schema").and_then(Value::as_str) != Some("vela.next_offer.v0.1") {
        return Err("work offer has an unsupported schema".to_string());
    }
    let target = offer
        .get("target")
        .and_then(Value::as_str)
        .filter(|target| !target.is_empty())
        .ok_or_else(|| "work offer has no target".to_string())?;
    let expected_session: String = target
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();
    if expected_session != session {
        return Err(format!(
            "work offer target {target} does not match session {session}"
        ));
    }
    let pinned = offer
        .get("pinned_state")
        .and_then(Value::as_object)
        .ok_or_else(|| "work offer has no pinned_state".to_string())?;
    let frontier_id = project.frontier_id();
    if pinned.get("frontier_id").and_then(Value::as_str) != Some(frontier_id.as_str()) {
        return Err("work offer belongs to a different frontier".to_string());
    }
    let pinned_event_hash = pinned
        .get("event_log_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| "work offer has no pinned event log hash".to_string())?;
    let receipt_event_root = context
        .get("event_log_root")
        .and_then(Value::as_str)
        .ok_or_else(|| "work session receipt has no event log root".to_string())?;
    if receipt_event_root != format!("sha256:{pinned_event_hash}") {
        return Err("work session receipt is not bound to its offered frontier state".to_string());
    }

    let lease = project
        .attempt_claims
        .iter()
        .find(|claim| claim.obligation_id == target)
        .ok_or_else(|| format!("work target {target} has no frontier lease"))?;
    if lease.claimant_actor != executor {
        return Err(format!(
            "work target {target} is leased by {}, not {executor}",
            lease.claimant_actor
        ));
    }
    let producer_key = context
        .get("identity_binding")
        .and_then(|value| value.get("public_key_hex"))
        .and_then(Value::as_str)
        .ok_or_else(|| "work session receipt has no producer public key".to_string())?;
    if lease.claimant_pubkey != producer_key {
        return Err("work session receipt key does not match the target lease key".to_string());
    }

    Ok(Some((offer_path, completed_path, target.to_string())))
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
    route: LandRoute,
    /// Decision-critical facts bound into the private transaction plan. Permit
    /// also stamps its policy context and certificate into the accepted event;
    /// Defer retains these bytes only for exact crash recovery and rederives a
    /// fresh brief at human review time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    review_route: Option<StagedReviewRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StagedReviewRoute {
    schema: String,
    policy_context: vela_protocol::acceptance_policy::PolicyContext,
    policy_decision: Option<vela_protocol::acceptance_policy::Decision>,
    policy_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    policy_authority_error: Option<String>,
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

#[derive(Debug, Clone)]
enum ProposalWriteAuthorization {
    /// A detached proposal signature supplied by a remote writer. The actor
    /// registry and signature are re-checked after the frontier-wide barrier
    /// is acquired.
    RegisteredSignature {
        canonical_signature: String,
        apply_if_tier_permits: bool,
    },
    /// A local CLI evidence writer that historically authored an unsigned
    /// draft. This mode may only insert a pending proposal; it can never apply
    /// one or cross the human decision boundary.
    LocalPendingDraft,
}

/// Persist one already signed MCP proposal through the same recoverable,
/// frontier-wide write edge used by receipt landing. Signature and tier checks
/// are repeated against the actor registry loaded while the barrier is held;
/// the in-memory MCP snapshot is never trusted as a write precondition.
pub(crate) fn transact_signed_proposal(
    frontier: &Path,
    proposal: vela_protocol::proposals::StateProposal,
    signature_hex: &str,
    apply_if_tier_permits: bool,
) -> Result<Value, String> {
    let signature_bytes = hex::decode(signature_hex)
        .map_err(|error| format!("invalid proposal signature hex: {error}"))?;
    if signature_bytes.len() != 64 {
        return Err("proposal signature must be 64 bytes".to_string());
    }
    transact_proposal_with_authorization(
        frontier,
        proposal,
        ProposalWriteAuthorization::RegisteredSignature {
            canonical_signature: hex::encode(signature_bytes),
            apply_if_tier_permits,
        },
        || Ok(()),
    )
}

/// Persist an unsigned proposal authored by a local CLI evidence command.
/// Creation is deliberately proposal-only: a key-custody human must later
/// decide it through the terminal ceremony.
pub(crate) fn transact_pending_proposal(
    frontier: &Path,
    proposal: vela_protocol::proposals::StateProposal,
) -> Result<Value, String> {
    transact_proposal_with_authorization(
        frontier,
        proposal,
        ProposalWriteAuthorization::LocalPendingDraft,
        || Ok(()),
    )
}

fn transact_proposal_with_authorization<F>(
    frontier: &Path,
    proposal: vela_protocol::proposals::StateProposal,
    authorization: ProposalWriteAuthorization,
    after_barrier: F,
) -> Result<Value, String>
where
    F: FnOnce() -> Result<(), String>,
{
    use crate::frontier_txn::{
        ContentDigest, DeltaDraft, FrontierBinding, FrontierTxn, FrontierTxnPlan,
        FrontierTxnPlanSpec, InputBinding, OperationId, OperationKind, PlannedWrite, RecoveryState,
        RepoPath,
    };

    let request = match &authorization {
        ProposalWriteAuthorization::RegisteredSignature {
            canonical_signature,
            apply_if_tier_permits,
        } => json!({
            "schema": "vela.signed-proposal-request.internal.v1",
            "proposal_id": &proposal.id,
            "signature": canonical_signature,
            "apply_if_tier_permits": apply_if_tier_permits,
        }),
        ProposalWriteAuthorization::LocalPendingDraft => json!({
            "schema": "vela.local-pending-proposal-request.internal.v1",
            "proposal_id": &proposal.id,
            "disposition": "pending_review",
        }),
    };
    let request_bytes = vela_protocol::canonical::to_canonical_bytes(&request)?;
    let request_root = ContentDigest::hash(&request_bytes);
    let operation_id = OperationId::derive("propose", request_root.as_str().as_bytes());
    let journal_dir = frontier_transaction_journal_dir(frontier)?;

    if let Some(mut existing) = FrontierTxn::open_if_present(frontier, &journal_dir, &operation_id)
        .map_err(|error| error.to_string())?
    {
        if existing.plan().request_root != request_root {
            return Err(format!(
                "operation {} is already bound to a different signed proposal",
                operation_id.as_str()
            ));
        }
        if !matches!(existing.recovery_state(), RecoveryState::Aborted) {
            let result = existing.plan().result.clone();
            if !matches!(existing.recovery_state(), RecoveryState::Completed) {
                existing
                    .mark_committed()
                    .map_err(|error| error.to_string())?;
                existing.install().map_err(|error| error.to_string())?;
                existing.complete().map_err(|error| error.to_string())?;
            }
            return Ok(result);
        }
        drop(existing);
    }

    let barrier = FrontierTxn::acquire_recovery_barrier(frontier, &journal_dir)
        .map_err(|error| error.to_string())?;
    after_barrier()?;
    let original = repo::load_from_path(frontier)?;
    let (applied, bind_actor_registry) = match &authorization {
        ProposalWriteAuthorization::RegisteredSignature {
            canonical_signature,
            apply_if_tier_permits,
        } => {
            let actor = original
                .actors
                .iter()
                .find(|actor| actor.id == proposal.actor.id)
                .ok_or_else(|| {
                    format!(
                        "actor '{}' is not registered in this frontier; register via `vela actor add` before writing",
                        proposal.actor.id
                    )
                })?;
            if actor.algorithm != "ed25519" {
                return Err(format!(
                    "actor '{}' uses unsupported signing algorithm '{}'",
                    proposal.actor.id, actor.algorithm
                ));
            }
            if actor.revoked_at.is_some() {
                return Err(format!(
                    "actor '{}' is revoked and may not create a new proposal",
                    proposal.actor.id
                ));
            }
            let tier_permits_apply =
                vela_protocol::sign::actor_can_auto_apply(actor, &proposal.kind);
            if *apply_if_tier_permits && !tier_permits_apply {
                let tier_label = actor.tier.as_deref().unwrap_or("none");
                return Err(format!(
                    "actor '{}' tier '{tier_label}' does not permit auto-apply for {}",
                    proposal.actor.id, proposal.kind
                ));
            }
            if !vela_protocol::sign::verify_proposal_signature(
                &proposal,
                canonical_signature,
                &actor.public_key,
            )? {
                return Err(format!(
                    "Signature does not verify for actor '{}' on this proposal",
                    proposal.actor.id
                ));
            }
            (*apply_if_tier_permits && tier_permits_apply, true)
        }
        ProposalWriteAuthorization::LocalPendingDraft => (false, false),
    };

    let mut candidate: vela_protocol::project::Project =
        serde_json::from_value(serde_json::to_value(&original).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let result =
        vela_protocol::proposals::create_or_apply_in_frontier(&mut candidate, proposal, applied)?;
    let result = json!({
        "proposal_id": result.proposal_id,
        "finding_id": result.finding_id,
        "status": result.status,
        "applied_event_id": result.applied_event_id,
    });

    let writes =
        PlannedWrite::from_managed_files(repo::render_vela_repo_files(frontier, &candidate)?)
            .map_err(|error| error.to_string())?;
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
    let mut resulting_event_ids = candidate
        .events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    resulting_event_ids.sort();
    let read_set = if bind_actor_registry {
        vec![
            InputBinding::existing_file(
                frontier,
                RepoPath::parse(".vela/actors.json").map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?,
        ]
    } else {
        Vec::new()
    };
    let fixed_time = chrono::Utc::now().to_rfc3339();
    let plan = FrontierTxnPlan::new(
        FrontierTxnPlanSpec {
            kind: OperationKind::Maintenance,
            operation_id,
            request_root,
            frontier: FrontierBinding::new(frontier, original.frontier_id(), &layout)
                .map_err(|error| error.to_string())?,
            fixed_time,
            expected_event_log_root,
            resulting_event_log_root,
            resulting_event_ids,
            read_set,
            result: result.clone(),
        },
        draft.delta.clone(),
    )
    .map_err(|error| error.to_string())?;
    let mut transaction = FrontierTxn::prepare_with_barrier(barrier, plan, draft)
        .map_err(|error| error.to_string())?;
    transaction
        .mark_committed()
        .map_err(|error| error.to_string())?;
    transaction.install().map_err(|error| error.to_string())?;
    transaction.complete().map_err(|error| error.to_string())?;
    Ok(result)
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
        PublishOptions::new(false, false)
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
        schema: "vela.staged-review-route.internal.v1".to_string(),
        policy_context: context.clone(),
        policy_decision: staged_policy_route.decision().cloned(),
        policy_state: staged_policy_route.policy_state().to_string(),
        policy_authority_error: staged_policy_route
            .authority_error()
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
            Err(PolicyLaneRefusal::Closed) => LandRoute::Deferred {
                reasons: vec!["no signed policy: every decision is the human's".to_string()],
            },
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
        schema: "vela.proposal-review-material.internal.v1".to_string(),
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
        route: route.clone(),
        review_route: Some(staged_review_route),
    };
    let work_session_close = active_work_session_close(
        frontier,
        &original,
        receipt,
        executor,
        operation_id.as_str(),
    )?;
    let mut writes =
        PlannedWrite::from_managed_files(repo::render_vela_repo_files(frontier, &candidate)?)
            .map_err(|error| error.to_string())?;
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
    if let Some((offer_path, completed_path, target)) = work_session_close {
        writes.push(PlannedWrite::delete(
            offer_path,
            WriteClass::PrivateCoordination,
        ));
        writes.push(PlannedWrite::write(
            completed_path,
            WriteClass::PrivateCoordination,
            pretty_json_bytes(&json!({
                "schema": "vela.work-session-completed.internal.v1",
                "target": target,
                "closed_at": &fixed_time,
                "operation_id": operation_id.as_str(),
                "receipt_root": &receipt_root,
                "record_id": &record.id,
                "proposal_id": &proposal_id,
                "route": &route,
            }))?,
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
    let canonical_frontier = frontier
        .canonicalize()
        .map_err(|error| format!("canonicalize frontier: {error}"))?;
    let mut records = Vec::with_capacity(values.len());
    let mut public_blobs = BTreeMap::<String, Vec<u8>>::new();
    let mut read_set = Vec::new();
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
        let local = local_candidate.then(|| canonical_frontier.join(relative));
        let local = local.filter(|candidate| candidate.exists());
        if let Some(local) = local {
            let metadata = std::fs::symlink_metadata(&local)
                .map_err(|error| format!("artifact {}: {error}", local.display()))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "artifact {} must be a regular non-symlink file",
                    local.display()
                ));
            }
            let canonical = local
                .canonicalize()
                .map_err(|error| format!("canonicalize artifact {}: {error}", local.display()))?;
            if !canonical.starts_with(&canonical_frontier) {
                return Err(format!(
                    "artifact {} escapes the frontier; use an immutable public URI or opaque restricted reference",
                    local.display()
                ));
            }
            let bytes = std::fs::read(&canonical)
                .map_err(|error| format!("read artifact {}: {error}", canonical.display()))?;
            let digest = hex::encode(Sha256::digest(&bytes));
            if declared_hash.is_some_and(|declared| declared != digest) {
                return Err(format!("artifact {index} sha256 does not match its bytes"));
            }
            if declared_size.is_some_and(|declared| declared != bytes.len() as u64) {
                return Err(format!(
                    "artifact {index} size_bytes does not match its bytes"
                ));
            }
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
                size_bytes: Some(metadata.len()),
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
        route,
        publication,
    }
}

#[cfg(test)]
mod transactional_proposal_tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use sha2::{Digest, Sha256};
    use vela_protocol::bundle::{
        Assertion, Conditions, Confidence, ConfidenceKind, ConfidenceMethod, Evidence, Extraction,
        FindingBundle, Flags, Provenance,
    };
    use vela_protocol::events::StateTarget;
    use vela_protocol::sign::ActorRecord;

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

    fn signed_note(
        target: &str,
        text: &str,
        key: &SigningKey,
    ) -> (vela_protocol::proposals::StateProposal, String) {
        let proposal = vela_protocol::proposals::new_proposal_at(
            "finding.note",
            StateTarget {
                r#type: "finding".to_string(),
                id: target.to_string(),
            },
            "reviewer:transaction-fixture",
            "human",
            "transaction fixture",
            json!({"text": text}),
            Vec::new(),
            Vec::new(),
            "2026-07-13T00:00:00Z",
        );
        let signature = vela_protocol::sign::sign_proposal(&proposal, key).unwrap();
        (proposal, signature)
    }

    fn race_receipt(frontier: &Path) -> ReceiptV1 {
        use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
        use vela_protocol::receipt_v1::{
            ArtifactInput, ProducerReportedRun, ReceiptBuilder, ReceiptInput,
        };

        let artifact_path = "artifacts/race-witness.json";
        let artifact = br#"{"race":"witness"}"#;
        std::fs::create_dir_all(frontier.join("artifacts")).unwrap();
        std::fs::write(frontier.join(artifact_path), artifact).unwrap();
        let digest = hex::encode(Sha256::digest(artifact));
        let project = repo::load_from_path(frontier).unwrap();
        let event_root = format!(
            "sha256:{}",
            vela_protocol::events::event_log_hash(&project.events)
        );
        let key = SigningKey::from_bytes(&[0x35; 32]);
        let at = "2026-07-14T02:00:00Z";
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:receipt-race".to_string(),
                actor_class: ActorClass::Agent,
                created_at: at.to_string(),
            },
            &key,
        )
        .unwrap();
        let operation_id = format!(
            "vop_{}",
            hex::encode(Sha256::digest(b"proposal-land-race-receipt"))
        );
        let input = ReceiptInput::new(
            "a receipt concurrent with a local evidence proposal".to_string(),
            "computational".to_string(),
            "exact".to_string(),
            vec![
                ArtifactInput::new(
                    artifact_path.to_string(),
                    "witness".to_string(),
                    Some(digest),
                    None,
                )
                .unwrap(),
            ],
            vec!["race fixture only".to_string()],
            vec![
                ProducerReportedRun::producer_reported(
                    "race-fixture".to_string(),
                    "pass".to_string(),
                )
                .unwrap(),
            ],
            "agent:receipt-race".to_string(),
            at.to_string(),
            event_root,
            ".".to_string(),
            operation_id,
            "urn:vela:policy:none".to_string(),
        )
        .unwrap();
        ReceiptBuilder::build(input, &identity).unwrap()
    }

    #[test]
    fn signed_proposals_share_the_land_recovery_barrier_and_retry_exactly() {
        let temp = tempfile::tempdir().unwrap();
        let key = SigningKey::from_bytes(&[0x66; 32]);
        let finding = finding();
        let target = finding.id.clone();
        let mut project = vela_protocol::project::assemble(
            "transactional-proposal",
            vec![finding],
            0,
            0,
            "fixture",
        );
        project.actors.push(ActorRecord {
            id: "reviewer:transaction-fixture".to_string(),
            public_key: hex::encode(key.verifying_key().to_bytes()),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-12T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        vela_protocol::repo::init_repo(temp.path(), &project).unwrap();
        let (first, first_signature) = signed_note(&target, "first note", &key);

        let journal_dir = frontier_transaction_journal_dir(temp.path()).unwrap();
        let barrier =
            crate::frontier_txn::FrontierTxn::acquire_recovery_barrier(temp.path(), &journal_dir)
                .unwrap();
        let busy = transact_signed_proposal(temp.path(), first.clone(), &first_signature, false)
            .unwrap_err();
        assert!(
            busy.contains("write lock") || busy.contains("busy"),
            "unexpected lock refusal: {busy}"
        );
        drop(barrier);

        let first_result =
            transact_signed_proposal(temp.path(), first, &first_signature, false).unwrap();
        let retry = transact_signed_proposal(
            temp.path(),
            vela_protocol::proposals::new_proposal_at(
                "finding.note",
                StateTarget {
                    r#type: "finding".to_string(),
                    id: target.clone(),
                },
                "reviewer:transaction-fixture",
                "human",
                "transaction fixture",
                json!({"text": "first note"}),
                Vec::new(),
                Vec::new(),
                "2026-07-14T00:00:00Z",
            ),
            &first_signature,
            false,
        )
        .unwrap();
        assert_eq!(retry, first_result);

        let (second, second_signature) = signed_note(&target, "second note", &key);
        let second_result =
            transact_signed_proposal(temp.path(), second, &second_signature, false).unwrap();
        assert_ne!(second_result["proposal_id"], first_result["proposal_id"]);
        let loaded = repo::load_from_path(temp.path()).unwrap();
        assert_eq!(loaded.proposals.len(), 2);
        assert!(
            loaded
                .proposals
                .iter()
                .any(|proposal| proposal.id == first_result["proposal_id"])
        );
        assert!(
            loaded
                .proposals
                .iter()
                .any(|proposal| proposal.id == second_result["proposal_id"])
        );
    }

    #[test]
    fn paused_local_proposal_and_concurrent_land_both_survive_after_retry() {
        let temp = tempfile::tempdir().unwrap();
        let finding = finding();
        let target = finding.id.clone();
        let project =
            vela_protocol::project::assemble("proposal-land-race", vec![finding], 0, 0, "fixture");
        vela_protocol::repo::init_repo(temp.path(), &project).unwrap();
        let receipt = race_receipt(temp.path());
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(temp.path())
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "-q"]);
        git(&["config", "user.name", "Vela Test"]);
        git(&["config", "user.email", "test@vela.invalid"]);
        git(&["config", "commit.gpgsign", "false"]);
        git(&["add", "-A"]);
        git(&["commit", "-qm", "baseline"]);
        let proposal = vela_protocol::proposals::new_proposal_at(
            "finding.note",
            StateTarget {
                r#type: "finding".to_string(),
                id: target,
            },
            "agent:local-evidence",
            "agent",
            "record local evidence without deciding it",
            json!({"text": "pending race note"}),
            Vec::new(),
            Vec::new(),
            "2026-07-14T02:01:00Z",
        );
        let proposal_id = proposal.id.clone();
        let root = temp.path().to_path_buf();
        let (reached_tx, reached_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let proposal_writer = std::thread::spawn(move || {
            transact_proposal_with_authorization(
                &root,
                proposal,
                ProposalWriteAuthorization::LocalPendingDraft,
                || {
                    reached_tx.send(()).unwrap();
                    resume_rx.recv().unwrap();
                    Ok(())
                },
            )
        });
        reached_rx.recv().unwrap();

        let busy = land(temp.path(), &receipt, "agent:receipt-race", false).unwrap_err();
        assert!(
            busy.contains("write lock") || busy.contains("busy"),
            "concurrent land should fail retryably at the shared barrier: {busy}"
        );
        resume_tx.send(()).unwrap();
        let proposal_result = proposal_writer.join().unwrap().unwrap();
        assert_eq!(proposal_result["status"], "pending_review");

        let land_result = land(temp.path(), &receipt, "agent:receipt-race", false).unwrap();
        let loaded = repo::load_from_path(temp.path()).unwrap();
        assert!(
            loaded
                .proposals
                .iter()
                .any(|candidate| candidate.id == proposal_id),
            "the local writer's pending proposal was lost"
        );
        assert!(
            loaded
                .proposals
                .iter()
                .any(|candidate| candidate.id == land_result.proposal_id),
            "the retried receipt landing was lost"
        );
    }
}
