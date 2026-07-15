//! Prepare-only recovery for prelaunch policy bytes.
//!
//! This module never accepts a proposal and never receives a private key. It
//! fingerprints one bounded legacy active pair, proves it has no authority
//! history, and inserts a closed pending governance proposal. The ordinary
//! isolated `vela sign` Decision Plan is the sole accepting ceremony.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use vela_protocol::acceptance_policy::{
    LegacyPolicyPairObservation, POLICY_JSON_MAX_BYTES, POLICY_SIGNATURE_JSON_MAX_BYTES,
    observe_legacy_policy_pair_bytes,
};
use vela_protocol::project::Project;
use vela_protocol::proposals::StateProposal;
use vela_protocol::proposals::policy_accept::{
    LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND, LEGACY_POLICY_RETIREMENT_SCHEMA,
    LegacyPolicyRetirementPayload, current_policy_head, ensure_legacy_policy_has_no_admissions,
    parse_legacy_policy_retirement_payload, validate_legacy_policy_retirement_proposal,
};

use crate::frontier_txn::{
    ContentDigest, DeltaDraft, FrontierBinding, FrontierRecoveryBarrier, FrontierTxn,
    FrontierTxnError, FrontierTxnPlan, FrontierTxnPlanSpec, InputBinding, OperationId,
    OperationKind, PlannedWrite, RecoveryOutcome, RepoPath,
};

const PREPARE_RESULT_SCHEMA: &str = "vela.policy-legacy-retirement-prepare.internal.v1";
const ACTIVE_POLICY_PATH: &str = ".vela/policies/active.json";
const ACTIVE_SIGNATURE_PATH: &str = ".vela/policies/active.sig.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LegacyRetirementPaths {
    pub(crate) observed_paths: Vec<String>,
    pub(crate) delete_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LegacyRetirementAudit {
    pub(crate) payload: LegacyPolicyRetirementPayload,
    pub(crate) paths: LegacyRetirementPaths,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct LegacyRetirementPrepareOutcome {
    pub(crate) schema: &'static str,
    pub(crate) proposal_id: String,
    pub(crate) status: String,
    pub(crate) policy_id: String,
    pub(crate) policy_bytes_root: String,
    pub(crate) signature_bytes_root: String,
    pub(crate) identical_snapshot_pair: bool,
    pub(crate) next: &'static str,
}

pub(crate) fn cmd_policy_retire_legacy(
    frontier: &Path,
    reason: &str,
    actor: &str,
    json_output: bool,
) {
    match prepare_legacy_policy_retirement_at(
        frontier,
        reason,
        actor,
        &Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
    ) {
        Ok(outcome) if json_output => crate::cli::print_json(&outcome),
        Ok(outcome) => {
            println!("prepared {}", outcome.proposal_id);
            println!("  legacy policy: {}", outcome.policy_id);
            println!("  status: {}", outcome.status);
            println!("  next: {}", outcome.next);
        }
        Err(error) => crate::ui::fail_with(
            crate::ui::ErrorKind::Domain,
            &error,
            Some(
                "repair the reported legacy-state inconsistency; do not delete policy files by hand",
            ),
        ),
    }
}

pub(crate) fn is_legacy_policy_retirement(proposal: &StateProposal) -> bool {
    proposal.kind == LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND
}

pub(crate) fn legacy_retirement_paths(
    proposal: &StateProposal,
) -> Result<LegacyRetirementPaths, String> {
    let payload = parse_legacy_policy_retirement_payload(proposal)?;
    let snapshot_policy = format!(".vela/policies/{}.json", payload.policy_id);
    let snapshot_signature = format!(".vela/policies/{}.sig.json", payload.policy_id);
    let observed_paths = vec![
        ACTIVE_POLICY_PATH.to_string(),
        ACTIVE_SIGNATURE_PATH.to_string(),
        snapshot_policy.clone(),
        snapshot_signature.clone(),
    ];
    let mut delete_paths = vec![
        ACTIVE_POLICY_PATH.to_string(),
        ACTIVE_SIGNATURE_PATH.to_string(),
    ];
    if payload.retire_identical_snapshot_pair {
        delete_paths.push(snapshot_policy);
        delete_paths.push(snapshot_signature);
    }
    Ok(LegacyRetirementPaths {
        observed_paths,
        delete_paths,
    })
}

/// Recheck every live precondition. Callers invoke this during preparation and
/// review derivation; the Decision Plan then binds all returned observed paths
/// in its read set and verifies them again before key access and commit.
pub(crate) fn audit_legacy_policy_retirement(
    frontier: &Path,
    project: &Project,
    proposal: &StateProposal,
) -> Result<LegacyRetirementAudit, String> {
    validate_legacy_policy_retirement_proposal(project, proposal)?;
    let payload = parse_legacy_policy_retirement_payload(proposal)?;
    if current_policy_head(project)?.is_some() {
        return Err(
            "legacy policy retirement is only available before the first signed policy head"
                .to_string(),
        );
    }
    ensure_legacy_policy_has_no_admissions(project, &payload.policy_id)?;
    let replay = vela_protocol::reducer::verify_replay(project);
    if !replay.ok {
        return Err(format!(
            "legacy policy retirement requires an intact event replay: {}",
            replay.diffs.join("; ")
        ));
    }

    let (policy_bytes, signature_bytes, observed) = read_active_pair(frontier)?;
    if observed.stored_policy_id != payload.policy_id
        || observed.policy_bytes_root != payload.policy_bytes_root
        || observed.signature_bytes_root != payload.signature_bytes_root
    {
        return Err(
            "legacy active policy bytes drifted from the content-bound retirement proposal"
                .to_string(),
        );
    }
    let paths = legacy_retirement_paths(proposal)?;
    let snapshot_policy = read_optional(
        frontier,
        Path::new(&paths.observed_paths[2]),
        POLICY_JSON_MAX_BYTES as u64,
        "legacy policy snapshot",
    )?;
    let snapshot_signature = read_optional(
        frontier,
        Path::new(&paths.observed_paths[3]),
        POLICY_SIGNATURE_JSON_MAX_BYTES as u64,
        "legacy policy signature snapshot",
    )?;
    match (
        payload.retire_identical_snapshot_pair,
        snapshot_policy,
        snapshot_signature,
    ) {
        (false, None, None) => {}
        (true, Some(policy), Some(signature))
            if policy == policy_bytes && signature == signature_bytes => {}
        (true, Some(_), Some(_)) => {
            return Err(
                "same-id legacy policy snapshots differ from the active byte pair".to_string(),
            );
        }
        (true, _, _) => {
            return Err(
                "retirement proposal expects one complete same-id legacy snapshot pair".to_string(),
            );
        }
        (false, _, _) => {
            return Err(
                "same-id legacy snapshots appeared after the retirement proposal was prepared"
                    .to_string(),
            );
        }
    }
    Ok(LegacyRetirementAudit { payload, paths })
}

pub(crate) fn prepare_legacy_policy_retirement_at(
    frontier: &Path,
    reason: &str,
    actor: &str,
    fixed_at: &str,
) -> Result<LegacyRetirementPrepareOutcome, String> {
    if reason.trim().is_empty() {
        return Err("legacy policy retirement requires a non-empty reason".to_string());
    }
    if !(actor.starts_with("agent:")
        || actor.starts_with("ci:")
        || actor.starts_with("reviewer:")
        || actor.starts_with("steward:"))
    {
        return Err(
            "legacy policy retirement --as must be a stable agent:, ci:, reviewer:, or steward: identity"
                .to_string(),
        );
    }
    chrono::DateTime::parse_from_rfc3339(fixed_at)
        .map_err(|error| format!("legacy retirement time must be RFC3339: {error}"))?;
    if !frontier.join(".vela").is_dir() {
        return Err("legacy policy retirement requires a directory Vela frontier".to_string());
    }

    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)?;
    let barrier = acquire_barrier_with_recovery(frontier, &journal_dir)?;
    let project = vela_protocol::repo::load_from_path(frontier)?;
    if current_policy_head(&project)?.is_some() {
        return Err(
            "legacy policy retirement is only available before the first signed policy head"
                .to_string(),
        );
    }
    let (policy_bytes, signature_bytes, observed) = read_active_pair(frontier)?;
    ensure_legacy_policy_has_no_admissions(&project, &observed.stored_policy_id)?;
    let replay = vela_protocol::reducer::verify_replay(&project);
    if !replay.ok {
        return Err(format!(
            "legacy policy retirement requires an intact event replay: {}",
            replay.diffs.join("; ")
        ));
    }
    let identical_snapshot_pair = discover_snapshot_pair(
        frontier,
        &observed.stored_policy_id,
        &policy_bytes,
        &signature_bytes,
    )?;
    let payload = LegacyPolicyRetirementPayload {
        schema: LEGACY_POLICY_RETIREMENT_SCHEMA.to_string(),
        policy_id: observed.stored_policy_id.clone(),
        policy_bytes_root: observed.policy_bytes_root.clone(),
        signature_bytes_root: observed.signature_bytes_root.clone(),
        retire_identical_snapshot_pair: identical_snapshot_pair,
    };
    let proposal = vela_protocol::proposals::new_proposal_at(
        LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND,
        vela_protocol::events::StateTarget {
            r#type: "governance".to_string(),
            id: project.frontier_id().to_string(),
        },
        actor,
        vela_protocol::events::actor_kind(actor),
        reason,
        serde_json::to_value(payload).map_err(|error| error.to_string())?,
        Vec::new(),
        vec![
            "This proposal fingerprints unsupported prelaunch bytes; it does not validate their signature or grant policy authority."
                .to_string(),
        ],
        fixed_at,
    );
    let audit = audit_legacy_policy_retirement(frontier, &project, &proposal)?;
    let proposal_id = proposal.id.clone();
    if let Some(existing) = project
        .proposals
        .iter()
        .find(|existing| existing.id == proposal_id)
    {
        return Ok(outcome(existing, &audit.payload));
    }

    let mut candidate: Project =
        serde_json::from_value(serde_json::to_value(&project).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    vela_protocol::proposals::insert_pending_in_frontier(&mut candidate, proposal)?;
    let writes = PlannedWrite::from_managed_files(vela_protocol::repo::render_vela_repo_files(
        frontier, &candidate,
    )?)
    .map_err(|error| error.to_string())?;
    let draft = DeltaDraft::prepare(frontier, writes).map_err(|error| error.to_string())?;
    let layout = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.frontier-layout.internal.v1",
        "frontier_id": project.frontier_id(),
        "paths": draft
            .delta
            .writes()
            .iter()
            .map(|write| write.path.as_str())
            .collect::<Vec<_>>(),
    }))
    .map_err(|error| error.to_string())?;
    let mut read_set =
        vec![InputBinding::project_snapshot(&project).map_err(|error| error.to_string())?];
    for path in &audit.paths.observed_paths {
        read_set.push(
            InputBinding::current_file(
                frontier,
                RepoPath::parse(path).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?,
        );
    }
    read_set.sort_by(|left, right| left.name.cmp(&right.name));
    read_set.dedup_by(|left, right| left.name == right.name && left.digest == right.digest);
    barrier
        .verify_read_set(&read_set)
        .map_err(|error| error.to_string())?;

    let intent = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": PREPARE_RESULT_SCHEMA,
        "frontier_id": project.frontier_id(),
        "proposal_id": proposal_id,
        "policy_id": audit.payload.policy_id,
        "policy_bytes_root": audit.payload.policy_bytes_root,
        "signature_bytes_root": audit.payload.signature_bytes_root,
        "retire_identical_snapshot_pair": audit.payload.retire_identical_snapshot_pair,
    }))
    .map_err(|error| error.to_string())?;
    let request_root = ContentDigest::hash(intent);
    let operation_id =
        OperationId::derive("legacy-policy-retirement-prepare", proposal_id.as_bytes());
    let event_log_root = ContentDigest::parse(format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    ))
    .map_err(|error| error.to_string())?;
    let mut resulting_event_ids = project
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
            frontier: FrontierBinding::new(frontier, project.frontier_id(), &layout)
                .map_err(|error| error.to_string())?,
            fixed_time: fixed_at.to_string(),
            expected_event_log_root: event_log_root.clone(),
            resulting_event_log_root: event_log_root,
            resulting_event_ids,
            read_set,
            result: json!({
                "schema": PREPARE_RESULT_SCHEMA,
                "proposal_id": proposal_id,
                "status": "pending_review",
            }),
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

    let installed = vela_protocol::repo::load_from_path(frontier)?;
    let proposal = installed
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| "legacy retirement transaction did not install its proposal".to_string())?;
    Ok(outcome(proposal, &audit.payload))
}

fn outcome(
    proposal: &StateProposal,
    payload: &LegacyPolicyRetirementPayload,
) -> LegacyRetirementPrepareOutcome {
    LegacyRetirementPrepareOutcome {
        schema: PREPARE_RESULT_SCHEMA,
        proposal_id: proposal.id.clone(),
        status: proposal.status.clone(),
        policy_id: payload.policy_id.clone(),
        policy_bytes_root: payload.policy_bytes_root.clone(),
        signature_bytes_root: payload.signature_bytes_root.clone(),
        identical_snapshot_pair: payload.retire_identical_snapshot_pair,
        next: "A registered human reviewer may inspect and decide this isolated proposal with `vela sign`; preparation read no key and changed no policy authority.",
    }
}

fn read_active_pair(
    frontier: &Path,
) -> Result<(Vec<u8>, Vec<u8>, LegacyPolicyPairObservation), String> {
    let policy = crate::bounded_file::read_bounded_frontier_file(
        frontier,
        Path::new(ACTIVE_POLICY_PATH),
        POLICY_JSON_MAX_BYTES as u64,
        "legacy active policy",
    )
    .map_err(|error| error.to_string())?;
    let signature = crate::bounded_file::read_bounded_frontier_file(
        frontier,
        Path::new(ACTIVE_SIGNATURE_PATH),
        POLICY_SIGNATURE_JSON_MAX_BYTES as u64,
        "legacy active policy signature",
    )
    .map_err(|error| error.to_string())?;
    let observation = observe_legacy_policy_pair_bytes(&policy, &signature)?;
    Ok((policy, signature, observation))
}

fn discover_snapshot_pair(
    frontier: &Path,
    policy_id: &str,
    active_policy: &[u8],
    active_signature: &[u8],
) -> Result<bool, String> {
    let policy_path = PathBuf::from(format!(".vela/policies/{policy_id}.json"));
    let signature_path = PathBuf::from(format!(".vela/policies/{policy_id}.sig.json"));
    let policy = read_optional(
        frontier,
        &policy_path,
        POLICY_JSON_MAX_BYTES as u64,
        "legacy policy snapshot",
    )?;
    let signature = read_optional(
        frontier,
        &signature_path,
        POLICY_SIGNATURE_JSON_MAX_BYTES as u64,
        "legacy policy signature snapshot",
    )?;
    match (policy, signature) {
        (None, None) => Ok(false),
        (Some(policy), Some(signature))
            if policy == active_policy && signature == active_signature =>
        {
            Ok(true)
        }
        (Some(_), Some(_)) => {
            Err("same-id legacy policy snapshots differ from the active byte pair".to_string())
        }
        _ => Err("same-id legacy policy snapshot pair is incomplete".to_string()),
    }
}

fn read_optional(
    frontier: &Path,
    path: &Path,
    max_bytes: u64,
    label: &str,
) -> Result<Option<Vec<u8>>, String> {
    match crate::bounded_file::read_bounded_frontier_file(frontier, path, max_bytes, label) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.code == "missing" => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn acquire_barrier_with_recovery(
    frontier: &Path,
    journal_dir: &Path,
) -> Result<FrontierRecoveryBarrier, String> {
    for _ in 0..3 {
        match FrontierTxn::acquire_recovery_barrier(frontier, journal_dir) {
            Ok(barrier) => return Ok(barrier),
            Err(FrontierTxnError::RecoveryRequired { operation_id, .. }) => {
                let operation_id =
                    OperationId::parse(operation_id).map_err(|error| error.to_string())?;
                match FrontierTxn::recover(frontier, journal_dir, &operation_id)
                    .map_err(|error| error.to_string())?
                {
                    RecoveryOutcome::Prepared => {
                        let mut transaction =
                            FrontierTxn::open(frontier, journal_dir, &operation_id)
                                .map_err(|error| error.to_string())?;
                        transaction
                            .abort_prepared()
                            .map_err(|error| error.to_string())?;
                    }
                    RecoveryOutcome::Aborted
                    | RecoveryOutcome::Completed
                    | RecoveryOutcome::AlreadyCompleted => {}
                }
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("frontier recovery did not reach a stable legacy-retirement barrier".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const ID: &str = "vap_e0abc750544408e637bd90e0661bac15";

    fn legacy_bytes() -> (Vec<u8>, Vec<u8>) {
        (
            format!(
                "{{\"schema\":\"vela.acceptance_policy.prelaunch\",\"id\":\"{ID}\",\"legacy\":true}}\n"
            )
            .into_bytes(),
            format!(
                "{{\"policy_id\":\"{ID}\",\"signature\":\"historical\",\"signed_at\":\"prelaunch\"}}\n"
            )
            .into_bytes(),
        )
    }

    fn fixture(with_snapshots: bool) -> (TempDir, Vec<u8>, Vec<u8>) {
        let temp = TempDir::new().unwrap();
        vela_protocol::frontier_repo::initialize(
            temp.path(),
            vela_protocol::frontier_repo::InitOptions {
                name: "legacy-retirement-test",
                initialize_git: false,
            },
        )
        .unwrap();
        let policies = temp.path().join(".vela/policies");
        std::fs::create_dir_all(&policies).unwrap();
        let (policy, signature) = legacy_bytes();
        std::fs::write(policies.join("active.json"), &policy).unwrap();
        std::fs::write(policies.join("active.sig.json"), &signature).unwrap();
        if with_snapshots {
            std::fs::write(policies.join(format!("{ID}.json")), &policy).unwrap();
            std::fs::write(policies.join(format!("{ID}.sig.json")), &signature).unwrap();
        }
        (temp, policy, signature)
    }

    #[test]
    fn prepare_is_keyless_idempotent_and_preserves_legacy_bytes() {
        let (temp, policy, signature) = fixture(true);
        let first = prepare_legacy_policy_retirement_at(
            temp.path(),
            "retire unsupported prelaunch bytes",
            "agent:test",
            "2026-07-15T00:00:00Z",
        )
        .unwrap();
        let second = prepare_legacy_policy_retirement_at(
            temp.path(),
            "retire unsupported prelaunch bytes",
            "agent:test",
            "2026-07-15T01:00:00Z",
        )
        .unwrap();
        assert_eq!(first.proposal_id, second.proposal_id);
        assert_eq!(first.status, "pending_review");
        assert!(first.identical_snapshot_pair);
        assert_eq!(
            std::fs::read(temp.path().join(ACTIVE_POLICY_PATH)).unwrap(),
            policy
        );
        assert_eq!(
            std::fs::read(temp.path().join(ACTIVE_SIGNATURE_PATH)).unwrap(),
            signature
        );
        let project = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        assert_eq!(
            project
                .proposals
                .iter()
                .filter(|proposal| is_legacy_policy_retirement(proposal))
                .count(),
            1
        );
    }

    #[test]
    fn prepare_postimage_is_byte_exact_after_official_materialization() {
        const REASON: &str = "retire unsupported prelaunch bytes";
        const PREPARED_AT: &str = "2026-07-15T00:00:00Z";
        let (temp, _, _) = fixture(true);
        let mut project = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        let (_, _, observed) = read_active_pair(temp.path()).unwrap();
        let payload = LegacyPolicyRetirementPayload {
            schema: LEGACY_POLICY_RETIREMENT_SCHEMA.to_string(),
            policy_id: observed.stored_policy_id,
            policy_bytes_root: observed.policy_bytes_root,
            signature_bytes_root: observed.signature_bytes_root,
            retire_identical_snapshot_pair: true,
        };
        let expected = vela_protocol::proposals::new_proposal_at(
            LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND,
            vela_protocol::events::StateTarget {
                r#type: "governance".to_string(),
                id: project.frontier_id().to_string(),
            },
            "agent:test",
            "agent",
            REASON,
            serde_json::to_value(&payload).unwrap(),
            Vec::new(),
            vec![
                "This proposal fingerprints unsupported prelaunch bytes; it does not validate their signature or grant policy authority."
                    .to_string(),
            ],
            PREPARED_AT,
        );
        let seed = (0..1024)
            .map(|nonce| {
                vela_protocol::proposals::new_proposal_at(
                    LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND,
                    vela_protocol::events::StateTarget {
                        r#type: "governance".to_string(),
                        id: project.frontier_id().to_string(),
                    },
                    "agent:seed",
                    "agent",
                    &format!("ordering seed {nonce}"),
                    serde_json::to_value(&payload).unwrap(),
                    Vec::new(),
                    vec!["ordering-only test fixture".to_string()],
                    "2026-07-14T00:00:00Z",
                )
            })
            .find(|proposal| proposal.id > expected.id)
            .expect("a deterministic seed should sort after the retirement proposal");
        assert!(expected.id < seed.id);
        project.proposals.push(seed);
        project
            .proposals
            .sort_by(|left, right| left.id.cmp(&right.id));
        vela_protocol::repo::save_to_path(temp.path(), &project).unwrap();

        let prepared =
            prepare_legacy_policy_retirement_at(temp.path(), REASON, "agent:test", PREPARED_AT)
                .unwrap();
        assert_eq!(prepared.proposal_id, expected.id);
        let prepared_frontier = std::fs::read(temp.path().join("frontier.json")).unwrap();
        let prepared_lock = std::fs::read(temp.path().join("vela.lock")).unwrap();
        let prepared_proof = std::fs::read(temp.path().join("proof/latest.json")).unwrap();
        let installed = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        assert_eq!(
            installed
                .proposals
                .iter()
                .position(|proposal| proposal.id == prepared.proposal_id),
            Some(0),
            "the new proposal must already occupy split-repository id order"
        );

        vela_protocol::frontier_repo::materialize(temp.path()).unwrap();
        assert_eq!(
            std::fs::read(temp.path().join("frontier.json")).unwrap(),
            prepared_frontier
        );
        assert_eq!(
            std::fs::read(temp.path().join("vela.lock")).unwrap(),
            prepared_lock
        );
        assert_eq!(
            std::fs::read(temp.path().join("proof/latest.json")).unwrap(),
            prepared_proof
        );

        let repeated = prepare_legacy_policy_retirement_at(
            temp.path(),
            REASON,
            "agent:test",
            "2026-07-15T01:00:00Z",
        )
        .unwrap();
        assert_eq!(repeated.proposal_id, prepared.proposal_id);
    }

    #[test]
    fn prepare_without_snapshots_binds_absence_and_deletes_only_active_pair() {
        let (temp, policy, signature) = fixture(false);
        let prepared = prepare_legacy_policy_retirement_at(
            temp.path(),
            "retire unsupported prelaunch bytes",
            "agent:test",
            "2026-07-15T00:00:00Z",
        )
        .unwrap();
        assert!(!prepared.identical_snapshot_pair);
        let project = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        let proposal = project
            .proposals
            .iter()
            .find(|proposal| proposal.id == prepared.proposal_id)
            .unwrap();
        let audit = audit_legacy_policy_retirement(temp.path(), &project, proposal).unwrap();
        assert_eq!(
            audit.paths.delete_paths,
            vec![ACTIVE_POLICY_PATH, ACTIVE_SIGNATURE_PATH]
        );
        assert_eq!(audit.paths.observed_paths.len(), 4);
        assert_eq!(
            std::fs::read(temp.path().join(ACTIVE_POLICY_PATH)).unwrap(),
            policy
        );
        assert_eq!(
            std::fs::read(temp.path().join(ACTIVE_SIGNATURE_PATH)).unwrap(),
            signature
        );
    }

    #[test]
    fn audit_rejects_snapshot_or_active_byte_drift() {
        let (temp, _, _) = fixture(true);
        let prepared = prepare_legacy_policy_retirement_at(
            temp.path(),
            "retire unsupported prelaunch bytes",
            "agent:test",
            "2026-07-15T00:00:00Z",
        )
        .unwrap();
        let project = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        let proposal = project
            .proposals
            .iter()
            .find(|proposal| proposal.id == prepared.proposal_id)
            .unwrap();
        std::fs::write(
            temp.path().join(format!(".vela/policies/{ID}.json")),
            b"different",
        )
        .unwrap();
        let error = audit_legacy_policy_retirement(temp.path(), &project, proposal).unwrap_err();
        assert!(error.contains("snapshots differ"), "{error}");
    }
}
