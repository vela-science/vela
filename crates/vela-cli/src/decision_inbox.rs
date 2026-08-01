//! Deterministic, read-only Decision Inbox projection.
//!
//! The Inbox is not a retained object or a second review lifecycle. Every
//! entry is rebuilt from the verified current repository, exact scientific
//! records, and active authority heads. Changing any reviewed input produces
//! a different entry root.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};
use vela_protocol::claim_record::ClaimRecordV1;
use vela_protocol::current_repository::{CurrentRepositoryV4, RepositoryObjectRefV1};
use vela_protocol::proposal_v1::ProposalV1;
use vela_protocol::repository_origin::RepositoryOriginV1;
use vela_protocol::submission_v1::SubmissionV1;
use vela_protocol::verification_record::VerificationRecordV1;

use crate::current_repository_decision::{
    DecisionAction, claim_for_proposal, exact_verifications, next_repository,
    submission_for_proposal, verification_satisfies_requirement, verification_set_root,
};

const ENTRY_SCHEMA: &str = "vela.decision-inbox-entry.v2";
const ENTRY_DOMAIN: &[u8] = b"vela.decision-inbox-entry.v2\0";
const PROJECTION_SCHEMA: &str = "vela.decision-inbox.v2";
const PROJECTION_DOMAIN: &[u8] = b"vela.decision-inbox.v2\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxAuthorityHeads {
    pub(crate) policy_bundle_root: String,
    pub(crate) authority_keyset_root: String,
    pub(crate) authority_record_root: String,
    pub(crate) authority_event_log_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxInputRoots {
    pub(crate) repository_root: String,
    pub(crate) proposal_root: String,
    pub(crate) claim_root: String,
    pub(crate) submission_root: String,
    pub(crate) verification_set_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxVerification {
    pub(crate) verification_record_id: String,
    pub(crate) verification_record_root: String,
    pub(crate) outcome: String,
    pub(crate) property: String,
    pub(crate) verifier: String,
    pub(crate) independent_of_producer: bool,
    pub(crate) satisfies_requirements: Vec<String>,
    pub(crate) protocol_evidence_role: String,
    pub(crate) does_not_establish: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxBlocker {
    pub(crate) code: String,
    pub(crate) subject: String,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxReadiness {
    pub(crate) protocol_gate: String,
    pub(crate) human_decision_required: bool,
    pub(crate) rejection_available: bool,
    pub(crate) blockers: Vec<DecisionInboxBlocker>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AcceptedStanding {
    pub(crate) claim_id: String,
    pub(crate) claim_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxStandingScope {
    pub(crate) kind: String,
    pub(crate) target_claim_id: String,
    pub(crate) affected_claim_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxStandingState {
    pub(crate) repository_root: String,
    pub(crate) accepted: Vec<AcceptedStanding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxGlobalAcceptedCounts {
    pub(crate) before: usize,
    pub(crate) if_accept: usize,
    pub(crate) if_reject: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxStandingCounts {
    pub(crate) unchanged_accepted_claims: usize,
    pub(crate) global_accepted_claims: DecisionInboxGlobalAcceptedCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxStandingDelta {
    pub(crate) transition: String,
    pub(crate) scope: DecisionInboxStandingScope,
    pub(crate) before: DecisionInboxStandingState,
    pub(crate) if_accept: DecisionInboxStandingState,
    pub(crate) if_reject: DecisionInboxStandingState,
    pub(crate) counts: DecisionInboxStandingCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxStaleness {
    pub(crate) state: String,
    pub(crate) invalidated_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxNextObligation {
    pub(crate) now: String,
    pub(crate) if_accept: String,
    pub(crate) if_reject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxEntry {
    pub(crate) schema: String,
    pub(crate) frontier_id: String,
    pub(crate) proposal_id: String,
    pub(crate) created_at: String,
    pub(crate) requested_decision: String,
    pub(crate) proposal_action: String,
    pub(crate) proposal_actor: String,
    pub(crate) proposal_reason: String,
    pub(crate) claim_id: String,
    pub(crate) assertion: String,
    pub(crate) conditions: Vec<String>,
    pub(crate) inputs: DecisionInboxInputRoots,
    pub(crate) authority_heads: DecisionInboxAuthorityHeads,
    pub(crate) verification_requirements: Vec<String>,
    pub(crate) verification_records: Vec<DecisionInboxVerification>,
    pub(crate) readiness: DecisionInboxReadiness,
    pub(crate) standing_delta: DecisionInboxStandingDelta,
    pub(crate) limits: Vec<String>,
    pub(crate) staleness: DecisionInboxStaleness,
    pub(crate) next_obligation: DecisionInboxNextObligation,
    pub(crate) entry_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxProjection {
    pub(crate) schema: String,
    pub(crate) frontier_id: String,
    pub(crate) repository_root: String,
    pub(crate) order: String,
    pub(crate) entries: Vec<DecisionInboxEntry>,
    pub(crate) projection_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionInboxRootComparison {
    pub(crate) state: String,
    pub(crate) requested_entry_root: String,
    pub(crate) current_entry_root: String,
}

struct EntryInputs<'a> {
    repository: &'a CurrentRepositoryV4,
    repository_root: &'a str,
    proposal_reference: &'a RepositoryObjectRefV1,
    proposal: &'a ProposalV1,
    claim: &'a ClaimRecordV1,
    submission: &'a SubmissionV1,
    verifications: &'a [(String, VerificationRecordV1)],
    pending_conflicts: &'a [String],
    authority_heads: &'a DecisionInboxAuthorityHeads,
}

fn acceptance_blockers(
    submission: &SubmissionV1,
    records: &[(String, VerificationRecordV1)],
    pending_conflicts: &[String],
) -> Vec<DecisionInboxBlocker> {
    let mut blockers = Vec::new();
    for (_, record) in records {
        if matches!(record.outcome.as_str(), "fail" | "error") {
            blockers.push(DecisionInboxBlocker {
                code: "failing_verification".into(),
                subject: record.verification_record_id.clone(),
                detail: format!(
                    "Verification {} reports {} for {}.",
                    record.verification_record_id, record.outcome, record.scope.property
                ),
            });
        }
    }
    for requirement in &submission.verification_requirements {
        let satisfied = records
            .iter()
            .any(|(_, record)| verification_satisfies_requirement(submission, requirement, record));
        if !satisfied {
            blockers.push(DecisionInboxBlocker {
                code: "missing_independent_passing_verification".into(),
                subject: requirement.clone(),
                detail: format!(
                    "No independent passing Verification Record satisfies {requirement:?}."
                ),
            });
        }
    }
    for proposal_id in pending_conflicts {
        blockers.push(DecisionInboxBlocker {
            code: "same_execution_pending".into(),
            subject: proposal_id.clone(),
            detail: format!(
                "Proposal {proposal_id} binds the same exact producer run, attempt, artifacts, scope, requested change, and verifier contract. Resolve one wording before acceptance."
            ),
        });
    }
    blockers.sort();
    blockers.dedup();
    blockers
}

fn classify_verification(
    submission: &SubmissionV1,
    root: &str,
    record: &VerificationRecordV1,
) -> DecisionInboxVerification {
    let satisfies_requirements = submission
        .verification_requirements
        .iter()
        .filter(|requirement| verification_satisfies_requirement(submission, requirement, record))
        .cloned()
        .collect::<Vec<_>>();
    let protocol_evidence_role = if matches!(record.outcome.as_str(), "fail" | "error") {
        "blocking"
    } else if satisfies_requirements.is_empty() {
        "complementary"
    } else {
        "requirement_satisfying"
    };
    DecisionInboxVerification {
        verification_record_id: record.verification_record_id.clone(),
        verification_record_root: root.into(),
        outcome: record.outcome.clone(),
        property: record.scope.property.clone(),
        verifier: record.verifier.clone(),
        independent_of_producer: record.verifier != submission.provenance.producer
            && record
                .independence
                .declared_independent_of
                .contains(&submission.provenance.producer),
        satisfies_requirements,
        protocol_evidence_role: protocol_evidence_role.into(),
        does_not_establish: record.scope.does_not_establish.clone(),
    }
}

fn accepted_subset(
    repository: &CurrentRepositoryV4,
    affected_claim_ids: &BTreeSet<String>,
) -> Vec<AcceptedStanding> {
    let mut values = repository
        .accepted_claims
        .iter()
        .filter(|reference| affected_claim_ids.contains(&reference.claim_id))
        .map(|reference| AcceptedStanding {
            claim_id: reference.claim_id.clone(),
            claim_root: reference.claim_root.clone(),
        })
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn accepted_outside_scope(
    repository: &CurrentRepositoryV4,
    affected_claim_ids: &BTreeSet<String>,
) -> Vec<AcceptedStanding> {
    let mut values = repository
        .accepted_claims
        .iter()
        .filter(|reference| !affected_claim_ids.contains(&reference.claim_id))
        .map(|reference| AcceptedStanding {
            claim_id: reference.claim_id.clone(),
            claim_root: reference.claim_root.clone(),
        })
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn semantic_transition(
    proposal: &ProposalV1,
    claim: &ClaimRecordV1,
) -> Result<(String, String, BTreeSet<String>), String> {
    let mut affected = BTreeSet::from([claim.claim_id.clone()]);
    match proposal.action.as_str() {
        "claim.add" => Ok((
            "add accepted Claim".into(),
            claim.claim_id.clone(),
            affected,
        )),
        "claim.revise" => {
            let predecessors = claim
                .relations
                .iter()
                .filter(|relation| matches!(relation.kind.as_str(), "corrects" | "supersedes"))
                .collect::<Vec<_>>();
            let [predecessor] = predecessors.as_slice() else {
                return Err(format!(
                    "Decision Inbox Proposal {} does not name exactly one corrected Claim",
                    proposal.proposal_id
                ));
            };
            affected.insert(predecessor.target_claim_id.clone());
            Ok((
                "supersede accepted Claim with corrected Claim".into(),
                predecessor.target_claim_id.clone(),
                affected,
            ))
        }
        "claim.withdraw" => Ok((
            "remove Claim from accepted Standing".into(),
            claim.claim_id.clone(),
            affected,
        )),
        other => Err(format!(
            "Decision Inbox Proposal {} uses unsupported action {other}",
            proposal.proposal_id
        )),
    }
}

fn unique_limits(
    proposal: &ProposalV1,
    submission: &SubmissionV1,
    records: &[(String, VerificationRecordV1)],
) -> Vec<String> {
    let mut limits = proposal
        .caveats
        .iter()
        .chain(&submission.caveats)
        .cloned()
        .collect::<BTreeSet<_>>();
    for (_, record) in records {
        limits.extend(record.scope.does_not_establish.iter().cloned());
    }
    limits.into_iter().collect()
}

fn entry_root(entry: &DecisionInboxEntry) -> Result<String, String> {
    let mut value = serde_json::to_value(entry)
        .map_err(|error| format!("encode Decision Inbox entry: {error}"))?;
    value
        .as_object_mut()
        .ok_or_else(|| "Decision Inbox entry must encode as an object".to_string())?
        .insert(
            "entry_root".into(),
            serde_json::Value::String(String::new()),
        );
    let mut digest = Sha256::new();
    digest.update(ENTRY_DOMAIN);
    digest.update(vela_protocol::canonical::to_canonical_bytes(&value)?);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

fn derive_entry(inputs: EntryInputs<'_>) -> Result<DecisionInboxEntry, String> {
    let proposal = inputs.proposal;
    let claim = inputs.claim;
    let submission = inputs.submission;
    let records = inputs.verifications;
    let actual_repository_root = inputs.repository.canonical_root()?;
    let claim_root = claim.canonical_root()?;
    let submission_root = submission.canonical_root()?;
    if inputs.repository_root != actual_repository_root
        || inputs.authority_heads.policy_bundle_root != inputs.repository.authority_policy_root
        || inputs.authority_heads.authority_keyset_root != inputs.repository.authority_keyset_root
        || inputs.proposal_reference.id != proposal.proposal_id
        || claim_root != proposal.subject.root
        || submission_root != proposal.producer_package.root
        || inputs.proposal_reference.root != proposal.canonical_root()?
    {
        return Err(format!(
            "Decision Inbox Proposal {} input roots disagree",
            proposal.proposal_id
        ));
    }

    let blockers = acceptance_blockers(submission, records, inputs.pending_conflicts);
    let accepted = next_repository(
        inputs.repository,
        proposal,
        claim,
        &claim_root,
        DecisionAction::Accept,
    )?;
    let rejected = next_repository(
        inputs.repository,
        proposal,
        claim,
        &claim_root,
        DecisionAction::Reject,
    )?;
    let (transition, target_claim_id, affected_claim_ids) = semantic_transition(proposal, claim)?;
    let unchanged_accepted = accepted_outside_scope(inputs.repository, &affected_claim_ids);
    if unchanged_accepted != accepted_outside_scope(&accepted, &affected_claim_ids)
        || unchanged_accepted != accepted_outside_scope(&rejected, &affected_claim_ids)
    {
        return Err(format!(
            "Decision Inbox Proposal {} changes accepted Standing outside its declared Claim scope",
            proposal.proposal_id
        ));
    }
    let verification_set_root = verification_set_root(records)?;
    let verification_records = records
        .iter()
        .map(|(root, record)| classify_verification(submission, root, record))
        .collect::<Vec<_>>();
    let protocol_gate = if blockers.is_empty() {
        "satisfied"
    } else {
        "blocked"
    };
    let now = if blockers.is_empty() {
        "Human repository authority may inspect and accept or reject this exact rooted entry."
            .into()
    } else {
        format!(
            "Resolve {} acceptance blocker(s), or reject the Proposal with an attributed reason.",
            blockers.len()
        )
    };
    let mut entry = DecisionInboxEntry {
        schema: ENTRY_SCHEMA.into(),
        frontier_id: inputs.repository.frontier_id.clone(),
        proposal_id: proposal.proposal_id.clone(),
        created_at: proposal.created_at.clone(),
        requested_decision: "accept_or_reject".into(),
        proposal_action: proposal.action.clone(),
        proposal_actor: proposal.actor.clone(),
        proposal_reason: proposal.reason.clone(),
        claim_id: claim.claim_id.clone(),
        assertion: claim.assertion.text.clone(),
        conditions: claim.conditions.clone(),
        inputs: DecisionInboxInputRoots {
            repository_root: inputs.repository_root.into(),
            proposal_root: inputs.proposal_reference.root.clone(),
            claim_root,
            submission_root,
            verification_set_root,
        },
        authority_heads: inputs.authority_heads.clone(),
        verification_requirements: submission.verification_requirements.clone(),
        verification_records,
        readiness: DecisionInboxReadiness {
            protocol_gate: protocol_gate.into(),
            human_decision_required: true,
            rejection_available: true,
            blockers,
        },
        standing_delta: DecisionInboxStandingDelta {
            transition,
            scope: DecisionInboxStandingScope {
                kind: "proposal_affected_claims".into(),
                target_claim_id,
                affected_claim_ids: affected_claim_ids.iter().cloned().collect(),
            },
            before: DecisionInboxStandingState {
                repository_root: inputs.repository_root.into(),
                accepted: accepted_subset(inputs.repository, &affected_claim_ids),
            },
            if_accept: DecisionInboxStandingState {
                repository_root: accepted.canonical_root()?,
                accepted: accepted_subset(&accepted, &affected_claim_ids),
            },
            if_reject: DecisionInboxStandingState {
                repository_root: rejected.canonical_root()?,
                accepted: accepted_subset(&rejected, &affected_claim_ids),
            },
            counts: DecisionInboxStandingCounts {
                unchanged_accepted_claims: unchanged_accepted.len(),
                global_accepted_claims: DecisionInboxGlobalAcceptedCounts {
                    before: inputs.repository.accepted_claims.len(),
                    if_accept: accepted.accepted_claims.len(),
                    if_reject: rejected.accepted_claims.len(),
                },
            },
        },
        limits: unique_limits(proposal, submission, records),
        staleness: DecisionInboxStaleness {
            state: "current".into(),
            invalidated_by: vec![
                "repository_root".into(),
                "proposal_root".into(),
                "claim_root".into(),
                "submission_root".into(),
                "verification_set_root".into(),
                "policy_bundle_root".into(),
                "authority_keyset_root".into(),
                "authority_record_root".into(),
                "authority_event_log_root".into(),
            ],
        },
        next_obligation: DecisionInboxNextObligation {
            now,
            if_accept: "Replay the accepted successor repository, recompute derived Target obligations, and expose the exact next valid Target.".into(),
            if_reject: "Replay the rejected successor repository and expose the next valid Target without changing accepted Standing.".into(),
        },
        entry_root: String::new(),
    };
    entry.entry_root = entry_root(&entry)?;
    Ok(entry)
}

fn projection_root(projection: &DecisionInboxProjection) -> Result<String, String> {
    let mut value = serde_json::to_value(projection)
        .map_err(|error| format!("encode Decision Inbox projection: {error}"))?;
    value
        .as_object_mut()
        .ok_or_else(|| "Decision Inbox projection must encode as an object".to_string())?
        .insert(
            "projection_root".into(),
            serde_json::Value::String(String::new()),
        );
    let mut digest = Sha256::new();
    digest.update(PROJECTION_DOMAIN);
    digest.update(vela_protocol::canonical::to_canonical_bytes(&value)?);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

fn sort_entries(entries: &mut [DecisionInboxEntry]) {
    entries.sort_by(|left, right| {
        let left_priority = usize::from(left.readiness.protocol_gate != "satisfied");
        let right_priority = usize::from(right.readiness.protocol_gate != "satisfied");
        left_priority
            .cmp(&right_priority)
            .then_with(|| left.created_at.cmp(&right.created_at))
            .then_with(|| left.proposal_id.cmp(&right.proposal_id))
    });
}

/// Rebuild the complete pending scientific Decision Inbox from exact current
/// repository state. This function performs no writes and has no side effects.
#[allow(dead_code)]
pub(crate) fn project(frontier: &Path) -> Result<DecisionInboxProjection, String> {
    let repository = crate::current_repository::load_current_repository_at(frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let origin = RepositoryOriginV1::parse(
        &fs::read(frontier.join(".vela/origin.json"))
            .map_err(|error| format!("read current repository origin: {error}"))?,
    )?;
    let authority = crate::cli::load_current_repository_authority(frontier, &repository, &origin)?;
    let standings =
        crate::current_repository::load_current_proposal_standings(frontier, &repository)?;
    let authority_heads = DecisionInboxAuthorityHeads {
        policy_bundle_root: repository.authority_policy_root.clone(),
        authority_keyset_root: repository.authority_keyset_root.clone(),
        authority_record_root: authority
            .verification
            .final_authority_record_root
            .clone()
            .ok_or_else(|| "Decision Inbox requires a current authority-record head".to_string())?,
        authority_event_log_root: authority.verification.final_event_log_root.clone(),
    };

    let mut entries = Vec::new();
    for proposal_reference in &repository.proposals {
        if standings.contains_key(&proposal_reference.id) {
            continue;
        }
        let proposal = crate::current_repository_decision::read_exact(
            frontier,
            &proposal_reference.path,
            &proposal_reference.root,
            ProposalV1::parse,
            ProposalV1::canonical_bytes,
        )?;
        if proposal.proposal_id != proposal_reference.id {
            return Err(format!(
                "Decision Inbox Proposal reference {} has the wrong identity",
                proposal_reference.id
            ));
        }
        let claim = claim_for_proposal(frontier, &repository, &proposal)?;
        let submission = submission_for_proposal(frontier, &repository, &proposal)?;
        let verifications =
            exact_verifications(frontier, &repository, &proposal, &claim, &submission)?;
        let pending_conflicts = crate::current_repository_decision::pending_submission_conflicts(
            frontier,
            &repository,
            &proposal,
            &submission,
        )?;
        entries.push(derive_entry(EntryInputs {
            repository: &repository,
            repository_root: &repository_root,
            proposal_reference,
            proposal: &proposal,
            claim: &claim,
            submission: &submission,
            verifications: &verifications,
            pending_conflicts: &pending_conflicts,
            authority_heads: &authority_heads,
        })?);
    }
    sort_entries(&mut entries);
    let mut projection = DecisionInboxProjection {
        schema: PROJECTION_SCHEMA.into(),
        frontier_id: repository.frontier_id,
        repository_root,
        order: "protocol_ready_first_then_created_at_asc_then_proposal_id".into(),
        entries,
        projection_root: String::new(),
    };
    projection.projection_root = projection_root(&projection)?;
    Ok(projection)
}

/// Compare a URL- or sidecar-bound entry root with the current derived entry.
/// The comparison never mutates or silently substitutes the requested root.
#[allow(dead_code)]
pub(crate) fn compare_entry_root(
    entry: &DecisionInboxEntry,
    requested_entry_root: &str,
) -> DecisionInboxRootComparison {
    DecisionInboxRootComparison {
        state: if requested_entry_root == entry.entry_root {
            "current"
        } else {
            "stale"
        }
        .into(),
        requested_entry_root: requested_entry_root.into(),
        current_entry_root: entry.entry_root.clone(),
    }
}

fn review_context_from_projection(
    projection: &DecisionInboxProjection,
    proposal_id: &str,
) -> serde_json::Value {
    let entry = projection
        .entries
        .iter()
        .find(|entry| entry.proposal_id == proposal_id);
    serde_json::json!({
        "projection_root": projection.projection_root,
        "entry": entry,
    })
}

/// Return the exact pending Decision Inbox entry for one Proposal together
/// with the root of the complete projection from which it was selected.
///
/// Terminal Proposals are intentionally absent from the pending Inbox and
/// therefore return a null entry under the still-current projection root.
pub(crate) fn review_context(
    frontier: &Path,
    proposal_id: &str,
) -> Result<serde_json::Value, String> {
    let projection = project(frontier)?;
    Ok(review_context_from_projection(&projection, proposal_id))
}

pub(crate) fn cmd_decision_inbox(frontier: &Path, json_output: bool) {
    crate::ui::set_mode("review.inbox", json_output);
    let projection = project(frontier).unwrap_or_else(|error| crate::cli::fail(&error));
    if json_output {
        crate::cli::print_json(&projection);
        return;
    }

    println!(
        "Decision Inbox · {} pending consequence{}",
        projection.entries.len(),
        if projection.entries.len() == 1 {
            ""
        } else {
            "s"
        }
    );
    println!("  Frontier: {}", projection.frontier_id);
    println!("  Repository: {}", projection.repository_root);
    println!("  Projection: {}", projection.projection_root);
    if projection.entries.is_empty() {
        println!("\nNo scientific Decision requires human review.");
        return;
    }

    for entry in &projection.entries {
        let requirement_satisfying = entry
            .verification_records
            .iter()
            .filter(|record| record.protocol_evidence_role == "requirement_satisfying")
            .count();
        let complementary = entry
            .verification_records
            .iter()
            .filter(|record| record.protocol_evidence_role == "complementary")
            .count();
        let blocking = entry
            .verification_records
            .iter()
            .filter(|record| record.protocol_evidence_role == "blocking")
            .count();
        let protocol_blockers = entry.readiness.blockers.len();
        let readiness = if entry.readiness.protocol_gate == "satisfied" {
            "protocol checks satisfied; human judgment required"
        } else {
            "protocol checks blocked; human judgment still required"
        };
        println!(
            "\n{} · {} · {}",
            readiness, entry.proposal_id, entry.proposal_action
        );
        println!("  {}", crate::cli::safe_text::inline(&entry.assertion));
        println!("  Change: {}", entry.standing_delta.transition);
        println!(
            "  Standing: {} affected accepted Claim{} now · {} if accepted · {} if rejected",
            entry.standing_delta.before.accepted.len(),
            if entry.standing_delta.before.accepted.len() == 1 {
                ""
            } else {
                "s"
            },
            entry.standing_delta.if_accept.accepted.len(),
            entry.standing_delta.if_reject.accepted.len()
        );
        println!(
            "  Evidence: {requirement_satisfying} requirement-satisfying · \
             {complementary} complementary · {blocking} blocking · \
             {protocol_blockers} protocol blocker{}",
            if protocol_blockers == 1 { "" } else { "s" }
        );
        println!("  Entry: {}", entry.entry_root);
    }
    println!(
        "\nProtocol readiness is not a recommendation. Verification does not change Standing."
    );
    println!(
        "\nInspect: vela review show . {} --json",
        projection.entries[0].proposal_id
    );
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ed25519_dalek::SigningKey;
    use vela_protocol::claim_record::{ClaimAssertion, ClaimRelation, ClaimSource};
    use vela_protocol::current_repository::{
        CURRENT_REPOSITORY_SCHEMA_V4, ClaimStandingRefV1, RepositoryObjectRefV1,
    };
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::proposal_v1::{ProposalProducerPackage, ProposalSubject};
    use vela_protocol::submission_v1::{
        RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft, SubmissionProvenance,
    };
    use vela_protocol::verification_record::{
        IndependenceDisclosure, VerificationMethod, VerificationRecordDraft, VerificationScope,
        VerificationSubject,
    };

    use super::*;

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn claim(assertion: &str, revision: u32, relations: Vec<ClaimRelation>) -> ClaimRecordV1 {
        ClaimRecordV1::build(
            revision,
            ClaimAssertion {
                text: assertion.into(),
                kind: "computational".into(),
            },
            vec!["Exact fixture scope.".into()],
            Vec::new(),
            vec![ClaimSource {
                kind: "fixture".into(),
                title: "Decision Inbox fixture".into(),
                locator: None,
                authors: vec!["Fixture author".into()],
                year: Some(2026),
            }],
            relations,
            "2026-07-30T00:00:00Z".into(),
            BTreeMap::new(),
        )
        .unwrap()
    }

    fn submission(requirement: &str) -> SubmissionV1 {
        let key = SigningKey::from_bytes(&[71_u8; 32]);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:fixture-producer".into(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-30T00:00:00Z".into(),
            },
            &key,
        )
        .unwrap();
        SubmissionV1::build(
            SubmissionDraft {
                claim: SubmissionClaim {
                    assertion: "A bounded fixture result.".into(),
                    claim_type: "computational".into(),
                    conditions: vec!["Exact fixture scope.".into()],
                },
                artifacts: vec![SubmissionArtifact {
                    kind: "witness".into(),
                    path: "witness.json".into(),
                    digest: root('a'),
                }],
                caveats: vec!["Does not establish an unrestricted result.".into()],
                replayability: "exact".into(),
                producer_checks: Vec::new(),
                verification_requirements: vec![requirement.into()],
                requested_change: RequestedChange {
                    kind: "add_claim".into(),
                    target: None,
                },
                provenance: SubmissionProvenance {
                    producer: "agent:fixture-producer".into(),
                    source_system: "fixture".into(),
                    source_attempt: None,
                    source_run: Some("run_fixture".into()),
                    emitted_at: "2026-07-30T00:00:00Z".into(),
                },
                execution_binding: None,
            },
            identity,
            &key,
        )
        .unwrap()
    }

    fn proposal(action: &str, claim: &ClaimRecordV1, submission: &SubmissionV1) -> ProposalV1 {
        ProposalV1::build(
            action.into(),
            ProposalSubject {
                kind: "claim".into(),
                id: claim.claim_id.clone(),
                root: claim.canonical_root().unwrap(),
            },
            submission.provenance.producer.clone(),
            "2026-07-30T00:00:01Z".into(),
            "Review the exact bounded fixture evidence.".into(),
            ProposalProducerPackage {
                kind: "submission_v1".into(),
                id: submission.submission_id.clone(),
                root: submission.canonical_root().unwrap(),
                path: format!(
                    "records/submissions/sha256/{}.json",
                    submission
                        .canonical_root()
                        .unwrap()
                        .trim_start_matches("sha256:")
                ),
            },
            vec!["Scientific acceptance remains separate.".into()],
        )
        .unwrap()
    }

    fn verification(
        proposal: &ProposalV1,
        submission: &SubmissionV1,
        property: &str,
        outcome: &str,
    ) -> VerificationRecordV1 {
        let key = SigningKey::from_bytes(&[72_u8; 32]);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "service:fixture-verifier".into(),
                actor_class: ActorClass::Org,
                created_at: "2026-07-30T00:00:00Z".into(),
            },
            &key,
        )
        .unwrap();
        VerificationRecordV1::build(
            VerificationRecordDraft {
                subject: VerificationSubject {
                    claim_id: proposal.subject.id.clone(),
                    artifact_ids: vec!["a".repeat(64)],
                    submission_id: submission.submission_id.clone(),
                    submission_root: submission.canonical_root().unwrap(),
                    proposal_id: proposal.proposal_id.clone(),
                },
                method: VerificationMethod {
                    profile: "fixture-v1".into(),
                    implementation: "fixture-verifier".into(),
                    environment_root: root('b'),
                },
                scope: VerificationScope {
                    property: property.into(),
                    does_not_establish: vec!["Scientific acceptance.".into()],
                },
                outcome: outcome.into(),
                verifier: "service:fixture-verifier".into(),
                independence: IndependenceDisclosure {
                    declared_independent_of: vec![submission.provenance.producer.clone()],
                    shared_dependencies: Vec::new(),
                },
                output_artifact_ids: Vec::new(),
                started_at: "2026-07-30T00:00:02Z".into(),
                completed_at: "2026-07-30T00:00:03Z".into(),
            },
            identity,
            &key,
        )
        .unwrap()
    }

    fn claim_standing(claim: &ClaimRecordV1, standing: &str) -> ClaimStandingRefV1 {
        let claim_root = claim.canonical_root().unwrap();
        ClaimStandingRefV1 {
            claim_id: claim.claim_id.clone(),
            claim_root: claim_root.clone(),
            standing: standing.into(),
            path: format!(
                "records/claims/sha256/{}.json",
                claim_root.trim_start_matches("sha256:")
            ),
        }
    }

    fn object_reference(
        schema: &str,
        id: &str,
        root: &str,
        directory: &str,
    ) -> RepositoryObjectRefV1 {
        RepositoryObjectRefV1 {
            schema: schema.into(),
            id: id.into(),
            root: root.into(),
            path: format!(
                "records/{directory}/sha256/{}.json",
                root.trim_start_matches("sha256:")
            ),
        }
    }

    fn repository(
        accepted: Vec<ClaimStandingRefV1>,
        pending: Vec<ClaimStandingRefV1>,
        proposal: &ProposalV1,
        submission: &SubmissionV1,
        verification: &VerificationRecordV1,
    ) -> CurrentRepositoryV4 {
        let proposal_root = proposal.canonical_root().unwrap();
        let submission_root = submission.canonical_root().unwrap();
        let verification_root = verification.canonical_root().unwrap();
        CurrentRepositoryV4 {
            schema: CURRENT_REPOSITORY_SCHEMA_V4.into(),
            frontier_id: "vfr_0123456789abcdef".into(),
            profile_root: root('1'),
            origin_id: "vro_0123456789abcdef".into(),
            origin_root: root('2'),
            accepted_claims: accepted,
            pending_claims: pending,
            proposals: vec![object_reference(
                vela_protocol::proposal_v1::PROPOSAL_V1_SCHEMA,
                &proposal.proposal_id,
                &proposal_root,
                "proposals",
            )],
            proposal_withdrawals: Vec::new(),
            submissions: vec![RepositoryObjectRefV1 {
                schema: vela_protocol::submission_v1::SUBMISSION_V1_SCHEMA.into(),
                id: submission.submission_id.clone(),
                root: submission_root.clone(),
                path: format!(
                    "records/submissions/sha256/{}.json",
                    submission_root.trim_start_matches("sha256:")
                ),
            }],
            verifications: vec![object_reference(
                vela_protocol::verification_record::VERIFICATION_RECORD_V1_SCHEMA,
                &verification.verification_record_id,
                &verification_root,
                "verifications",
            )],
            artifacts: Vec::new(),
            authority_keyset_root: root('3'),
            authority_policy_root: root('4'),
        }
    }

    fn heads() -> DecisionInboxAuthorityHeads {
        DecisionInboxAuthorityHeads {
            policy_bundle_root: root('4'),
            authority_keyset_root: root('3'),
            authority_record_root: root('5'),
            authority_event_log_root: root('6'),
        }
    }

    fn derive_fixture(
        repository: &CurrentRepositoryV4,
        proposal: &ProposalV1,
        claim: &ClaimRecordV1,
        submission: &SubmissionV1,
        verification: &VerificationRecordV1,
        authority_heads: &DecisionInboxAuthorityHeads,
    ) -> DecisionInboxEntry {
        let proposal_reference = repository.proposals.first().unwrap();
        derive_entry(EntryInputs {
            repository,
            repository_root: &repository.canonical_root().unwrap(),
            proposal_reference,
            proposal,
            claim,
            submission,
            verifications: &[(verification.canonical_root().unwrap(), verification.clone())],
            pending_conflicts: &[],
            authority_heads,
        })
        .unwrap()
    }

    #[test]
    fn ready_entry_binds_exact_inputs_and_hypothetical_standing() {
        let requirement = "Replay the exact fixture.";
        let subject = claim("A bounded fixture result.", 1, Vec::new());
        let submission = submission(requirement);
        let proposal = proposal("claim.add", &subject, &submission);
        let verification = verification(&proposal, &submission, requirement, "pass");
        let repository = repository(
            Vec::new(),
            vec![claim_standing(&subject, "pending_review")],
            &proposal,
            &submission,
            &verification,
        );
        let entry = derive_fixture(
            &repository,
            &proposal,
            &subject,
            &submission,
            &verification,
            &heads(),
        );

        assert_eq!(entry.readiness.protocol_gate, "satisfied");
        assert!(entry.readiness.human_decision_required);
        assert!(entry.readiness.rejection_available);
        assert!(entry.readiness.blockers.is_empty());
        assert!(
            crate::current_repository_decision::require_acceptance_evidence(
                &submission,
                &[(verification.canonical_root().unwrap(), verification.clone())],
            )
            .is_ok()
        );
        assert_eq!(entry.standing_delta.scope.kind, "proposal_affected_claims");
        assert_eq!(
            entry.standing_delta.scope.affected_claim_ids,
            vec![subject.claim_id.clone()]
        );
        assert!(entry.standing_delta.before.accepted.is_empty());
        assert_eq!(
            entry.standing_delta.if_accept.accepted,
            vec![AcceptedStanding {
                claim_id: subject.claim_id.clone(),
                claim_root: subject.canonical_root().unwrap(),
            }]
        );
        assert!(entry.standing_delta.if_reject.accepted.is_empty());
        assert_eq!(entry.standing_delta.counts.unchanged_accepted_claims, 0);
        assert_eq!(entry.standing_delta.counts.global_accepted_claims.before, 0);
        assert_eq!(
            entry.standing_delta.counts.global_accepted_claims.if_accept,
            1
        );
        assert_eq!(
            entry.standing_delta.counts.global_accepted_claims.if_reject,
            0
        );
        assert_eq!(
            entry.inputs.proposal_root,
            proposal.canonical_root().unwrap()
        );
        assert_eq!(
            entry.inputs.verification_set_root,
            verification_set_root(&[(
                verification.canonical_root().unwrap(),
                verification.clone()
            )])
            .unwrap()
        );
        assert_eq!(entry.entry_root, entry_root(&entry).unwrap());
        assert_eq!(entry.authority_heads, heads());
        assert_eq!(
            entry.verification_records[0].protocol_evidence_role,
            "requirement_satisfying"
        );
        assert_eq!(
            entry.verification_records[0].satisfies_requirements,
            vec![requirement]
        );
    }

    #[test]
    fn inbox_places_actionable_decisions_before_blocked_cleanup() {
        let requirement = "Replay the exact fixture.";
        let subject = claim("A bounded fixture result.", 1, Vec::new());
        let submission = submission(requirement);
        let proposal = proposal("claim.add", &subject, &submission);
        let verification = verification(&proposal, &submission, requirement, "pass");
        let repository = repository(
            Vec::new(),
            vec![claim_standing(&subject, "pending_review")],
            &proposal,
            &submission,
            &verification,
        );
        let mut ready = derive_fixture(
            &repository,
            &proposal,
            &subject,
            &submission,
            &verification,
            &heads(),
        );
        ready.proposal_id = "vpr_ready000000000".into();
        ready.created_at = "2026-07-31T01:00:00Z".into();

        let mut blocked = ready.clone();
        blocked.proposal_id = "vpr_blocked000000".into();
        blocked.created_at = "2026-07-30T01:00:00Z".into();
        blocked.readiness.protocol_gate = "blocked".into();

        let mut entries = vec![blocked, ready];
        sort_entries(&mut entries);

        assert_eq!(entries[0].proposal_id, "vpr_ready000000000");
        assert_eq!(entries[1].proposal_id, "vpr_blocked000000");
    }

    #[test]
    fn review_context_embeds_the_matching_root_bound_entry() {
        let requirement = "Replay the exact fixture.";
        let subject = claim("A bounded fixture result.", 1, Vec::new());
        let submission = submission(requirement);
        let proposal = proposal("claim.add", &subject, &submission);
        let verification = verification(&proposal, &submission, requirement, "pass");
        let repository = repository(
            Vec::new(),
            vec![claim_standing(&subject, "pending_review")],
            &proposal,
            &submission,
            &verification,
        );
        let entry = derive_fixture(
            &repository,
            &proposal,
            &subject,
            &submission,
            &verification,
            &heads(),
        );
        let mut projection = DecisionInboxProjection {
            schema: PROJECTION_SCHEMA.into(),
            frontier_id: repository.frontier_id.clone(),
            repository_root: repository.canonical_root().unwrap(),
            order: "protocol_ready_first_then_created_at_asc_then_proposal_id".into(),
            entries: vec![entry.clone()],
            projection_root: String::new(),
        };
        projection.projection_root = projection_root(&projection).unwrap();

        let context = review_context_from_projection(&projection, &proposal.proposal_id);
        assert_eq!(context["projection_root"], projection.projection_root);
        assert_eq!(context["entry"]["entry_root"], entry.entry_root);
        assert_eq!(context["entry"]["readiness"]["protocol_gate"], "satisfied");
        assert_eq!(
            context["entry"]["standing_delta"]["if_accept"]["accepted"][0]["claim_id"],
            subject.claim_id
        );
        assert_eq!(context["entry"]["staleness"]["state"], "current");
        assert!(
            context["entry"]["next_obligation"]["if_accept"]
                .as_str()
                .is_some_and(|next| next.contains("Replay"))
        );

        let absent = review_context_from_projection(&projection, "vpr_0000000000000000");
        assert_eq!(absent["projection_root"], projection.projection_root);
        assert!(absent["entry"].is_null());
    }

    #[test]
    fn exact_records_distinguish_requirement_complementary_and_blocking_roles() {
        let requirement = "Replay the exact fixture.";
        let subject = claim("A bounded fixture result.", 1, Vec::new());
        let submission = submission(requirement);
        let proposal = proposal("claim.add", &subject, &submission);
        let satisfying = verification(&proposal, &submission, requirement, "pass");
        let complementary = verification(
            &proposal,
            &submission,
            "Inspect a complementary property.",
            "pass",
        );
        let blocking = verification(
            &proposal,
            &submission,
            "Inspect a complementary property.",
            "fail",
        );

        let satisfying = classify_verification(&submission, &root('1'), &satisfying);
        let complementary = classify_verification(&submission, &root('2'), &complementary);
        let blocking = classify_verification(&submission, &root('3'), &blocking);

        assert_eq!(satisfying.protocol_evidence_role, "requirement_satisfying");
        assert_eq!(satisfying.satisfies_requirements, vec![requirement]);
        assert_eq!(complementary.protocol_evidence_role, "complementary");
        assert!(complementary.satisfies_requirements.is_empty());
        assert_eq!(blocking.protocol_evidence_role, "blocking");
        assert!(blocking.satisfies_requirements.is_empty());
    }

    #[test]
    fn blockers_are_explicit_without_blocking_rejection() {
        let required = "Replay the exact fixture.";
        let subject = claim("A bounded fixture result.", 1, Vec::new());
        let submission = submission(required);
        let proposal = proposal("claim.add", &subject, &submission);
        let verification = verification(&proposal, &submission, "Another check.", "fail");
        let repository = repository(
            Vec::new(),
            vec![claim_standing(&subject, "pending_review")],
            &proposal,
            &submission,
            &verification,
        );
        let entry = derive_fixture(
            &repository,
            &proposal,
            &subject,
            &submission,
            &verification,
            &heads(),
        );

        assert_eq!(entry.readiness.protocol_gate, "blocked");
        assert!(entry.readiness.human_decision_required);
        assert!(entry.readiness.rejection_available);
        assert!(
            crate::current_repository_decision::require_acceptance_evidence(
                &submission,
                &[(verification.canonical_root().unwrap(), verification.clone())],
            )
            .is_err()
        );
        assert_eq!(
            entry
                .readiness
                .blockers
                .iter()
                .map(|blocker| blocker.code.as_str())
                .collect::<Vec<_>>(),
            vec![
                "failing_verification",
                "missing_independent_passing_verification"
            ]
        );
        assert!(entry.next_obligation.now.contains("2 acceptance blocker"));
    }

    #[test]
    fn same_execution_wording_retry_blocks_acceptance_without_hiding_rejection() {
        let requirement = "Replay the exact fixture.";
        let subject = claim("A bounded fixture result.", 1, Vec::new());
        let submission = submission(requirement);
        let proposal = proposal("claim.add", &subject, &submission);
        let verification = verification(&proposal, &submission, requirement, "pass");
        let repository = repository(
            Vec::new(),
            vec![claim_standing(&subject, "pending_review")],
            &proposal,
            &submission,
            &verification,
        );
        let proposal_reference = repository.proposals.first().unwrap();
        let entry = derive_entry(EntryInputs {
            repository: &repository,
            repository_root: &repository.canonical_root().unwrap(),
            proposal_reference,
            proposal: &proposal,
            claim: &subject,
            submission: &submission,
            verifications: &[(verification.canonical_root().unwrap(), verification)],
            pending_conflicts: &["vpr_correctedwording".into()],
            authority_heads: &heads(),
        })
        .unwrap();

        assert_eq!(entry.readiness.protocol_gate, "blocked");
        assert!(entry.readiness.rejection_available);
        assert_eq!(entry.readiness.blockers.len(), 1);
        assert_eq!(entry.readiness.blockers[0].code, "same_execution_pending");
        assert_eq!(entry.readiness.blockers[0].subject, "vpr_correctedwording");
    }

    #[test]
    fn correction_diff_replaces_only_the_exact_predecessor() {
        let original = claim("Original bounded result.", 1, Vec::new());
        let unrelated = claim("Unrelated accepted result.", 1, Vec::new());
        let correction = claim(
            "Corrected bounded result.",
            2,
            vec![ClaimRelation {
                kind: "corrects".into(),
                target_claim_id: original.claim_id.clone(),
            }],
        );
        let requirement = "Replay the correction.";
        let submission = submission(requirement);
        let proposal = proposal("claim.revise", &correction, &submission);
        let verification = verification(&proposal, &submission, requirement, "pass");
        let repository = repository(
            vec![
                claim_standing(&original, "accepted"),
                claim_standing(&unrelated, "accepted"),
            ],
            vec![claim_standing(&correction, "pending_review")],
            &proposal,
            &submission,
            &verification,
        );
        let entry = derive_fixture(
            &repository,
            &proposal,
            &correction,
            &submission,
            &verification,
            &heads(),
        );

        assert_eq!(entry.standing_delta.scope.kind, "proposal_affected_claims");
        assert_eq!(
            entry.standing_delta.scope.target_claim_id,
            original.claim_id
        );
        let mut affected_claim_ids = vec![correction.claim_id.clone(), original.claim_id.clone()];
        affected_claim_ids.sort();
        assert_eq!(
            entry.standing_delta.scope.affected_claim_ids,
            affected_claim_ids
        );
        assert_eq!(
            entry.standing_delta.before.accepted[0].claim_id,
            original.claim_id
        );
        assert_eq!(
            entry.standing_delta.if_accept.accepted[0].claim_id,
            correction.claim_id
        );
        assert_eq!(
            entry.standing_delta.if_reject.accepted[0].claim_id,
            original.claim_id
        );
        assert_eq!(entry.standing_delta.counts.unchanged_accepted_claims, 1);
        assert_eq!(entry.standing_delta.counts.global_accepted_claims.before, 2);
        assert_eq!(
            entry.standing_delta.counts.global_accepted_claims.if_accept,
            2
        );
        assert_eq!(
            entry.standing_delta.counts.global_accepted_claims.if_reject,
            2
        );
        assert_ne!(
            entry.standing_delta.if_accept.repository_root,
            entry.standing_delta.if_reject.repository_root
        );
    }

    #[test]
    fn withdrawal_delta_removes_only_the_exact_accepted_claim() {
        let subject = claim("Withdraw this bounded result.", 1, Vec::new());
        let unrelated = claim("Keep this unrelated result.", 1, Vec::new());
        let requirement = "Inspect the exact withdrawal evidence.";
        let submission = submission(requirement);
        let proposal = proposal("claim.withdraw", &subject, &submission);
        let verification = verification(&proposal, &submission, requirement, "pass");
        let repository = repository(
            vec![
                claim_standing(&subject, "accepted"),
                claim_standing(&unrelated, "accepted"),
            ],
            Vec::new(),
            &proposal,
            &submission,
            &verification,
        );
        let entry = derive_fixture(
            &repository,
            &proposal,
            &subject,
            &submission,
            &verification,
            &heads(),
        );

        assert_eq!(
            entry.standing_delta.scope.affected_claim_ids,
            vec![subject.claim_id.clone()]
        );
        assert_eq!(entry.standing_delta.before.accepted.len(), 1);
        assert!(entry.standing_delta.if_accept.accepted.is_empty());
        assert_eq!(entry.standing_delta.if_reject.accepted.len(), 1);
        assert_eq!(entry.standing_delta.counts.unchanged_accepted_claims, 1);
        assert_eq!(entry.standing_delta.counts.global_accepted_claims.before, 2);
        assert_eq!(
            entry.standing_delta.counts.global_accepted_claims.if_accept,
            1
        );
        assert_eq!(
            entry.standing_delta.counts.global_accepted_claims.if_reject,
            2
        );
    }

    #[test]
    fn every_authority_head_changes_the_entry_root_and_marks_old_links_stale() {
        let requirement = "Replay the exact fixture.";
        let subject = claim("A bounded fixture result.", 1, Vec::new());
        let submission = submission(requirement);
        let proposal = proposal("claim.add", &subject, &submission);
        let verification = verification(&proposal, &submission, requirement, "pass");
        let repository = repository(
            Vec::new(),
            vec![claim_standing(&subject, "pending_review")],
            &proposal,
            &submission,
            &verification,
        );
        let baseline = derive_fixture(
            &repository,
            &proposal,
            &subject,
            &submission,
            &verification,
            &heads(),
        );

        let mut changed_entries = Vec::new();
        let mut changed_repository = repository.clone();
        changed_repository.authority_policy_root = root('7');
        let mut changed_heads = heads();
        changed_heads.policy_bundle_root = root('7');
        changed_entries.push(derive_fixture(
            &changed_repository,
            &proposal,
            &subject,
            &submission,
            &verification,
            &changed_heads,
        ));

        let mut changed_repository = repository.clone();
        changed_repository.authority_keyset_root = root('8');
        let mut changed_heads = heads();
        changed_heads.authority_keyset_root = root('8');
        changed_entries.push(derive_fixture(
            &changed_repository,
            &proposal,
            &subject,
            &submission,
            &verification,
            &changed_heads,
        ));

        for mutate in [
            |heads: &mut DecisionInboxAuthorityHeads| heads.authority_record_root = root('9'),
            |heads: &mut DecisionInboxAuthorityHeads| heads.authority_event_log_root = root('a'),
        ] {
            let mut changed_heads = heads();
            mutate(&mut changed_heads);
            changed_entries.push(derive_fixture(
                &repository,
                &proposal,
                &subject,
                &submission,
                &verification,
                &changed_heads,
            ));
        }

        for changed in changed_entries {
            assert_ne!(baseline.entry_root, changed.entry_root);
            let comparison = compare_entry_root(&changed, &baseline.entry_root);
            assert_eq!(comparison.state, "stale");
            assert_eq!(comparison.requested_entry_root, baseline.entry_root);
            assert_eq!(comparison.current_entry_root, changed.entry_root);
        }
    }

    #[test]
    fn every_scientific_input_root_is_part_of_entry_identity() {
        let requirement = "Replay the exact fixture.";
        let subject = claim("A bounded fixture result.", 1, Vec::new());
        let submission = submission(requirement);
        let proposal = proposal("claim.add", &subject, &submission);
        let verification = verification(&proposal, &submission, requirement, "pass");
        let repository = repository(
            Vec::new(),
            vec![claim_standing(&subject, "pending_review")],
            &proposal,
            &submission,
            &verification,
        );
        let baseline = derive_fixture(
            &repository,
            &proposal,
            &subject,
            &submission,
            &verification,
            &heads(),
        );

        for mutate in [
            |entry: &mut DecisionInboxEntry| entry.inputs.repository_root = root('7'),
            |entry: &mut DecisionInboxEntry| entry.inputs.proposal_root = root('8'),
            |entry: &mut DecisionInboxEntry| entry.inputs.claim_root = root('9'),
            |entry: &mut DecisionInboxEntry| entry.inputs.submission_root = root('a'),
            |entry: &mut DecisionInboxEntry| entry.inputs.verification_set_root = root('b'),
        ] {
            let mut changed = baseline.clone();
            mutate(&mut changed);
            assert_ne!(entry_root(&changed).unwrap(), baseline.entry_root);
        }
    }

    #[test]
    fn limits_are_deduplicated_and_verification_never_becomes_acceptance() {
        let requirement = "Replay the exact fixture.";
        let subject = claim("A bounded fixture result.", 1, Vec::new());
        let submission = submission(requirement);
        let proposal = proposal("claim.add", &subject, &submission);
        let verification = verification(&proposal, &submission, requirement, "pass");
        let repository = repository(
            Vec::new(),
            vec![claim_standing(&subject, "pending_review")],
            &proposal,
            &submission,
            &verification,
        );
        let entry = derive_fixture(
            &repository,
            &proposal,
            &subject,
            &submission,
            &verification,
            &heads(),
        );

        assert_eq!(entry.requested_decision, "accept_or_reject");
        assert_eq!(entry.staleness.state, "current");
        assert!(entry.limits.contains(&"Scientific acceptance.".into()));
        assert_eq!(
            entry
                .limits
                .iter()
                .filter(|limit| limit.as_str() == "Scientific acceptance.")
                .count(),
            1
        );
        assert!(
            entry
                .next_obligation
                .now
                .starts_with("Human repository authority")
        );
    }
}
