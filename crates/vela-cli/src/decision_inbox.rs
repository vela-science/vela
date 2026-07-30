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
use vela_protocol::current_repository::{CurrentRepositoryV3, RepositoryObjectRefV1};
use vela_protocol::proposal_v1::ProposalV1;
use vela_protocol::repository_origin::RepositoryOriginV1;
use vela_protocol::submission_v1::SubmissionV1;
use vela_protocol::verification_record::VerificationRecordV1;

use crate::current_repository_decision::{
    DecisionAction, claim_for_proposal, exact_verifications, next_repository,
    submission_for_proposal, verification_set_root,
};

const ENTRY_SCHEMA: &str = "vela.decision-inbox-entry.v1";
const ENTRY_DOMAIN: &[u8] = b"vela.decision-inbox-entry.v1\0";
const PROJECTION_SCHEMA: &str = "vela.decision-inbox.v1";
const PROJECTION_DOMAIN: &[u8] = b"vela.decision-inbox.v1\0";

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
    pub(crate) acceptance: String,
    pub(crate) rejection: String,
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
pub(crate) struct DecisionInboxStandingDiff {
    pub(crate) transition: String,
    pub(crate) target_claim_id: String,
    pub(crate) accepted_before: Vec<AcceptedStanding>,
    pub(crate) accepted_if_accept: Vec<AcceptedStanding>,
    pub(crate) accepted_if_reject: Vec<AcceptedStanding>,
    pub(crate) repository_root_if_accept: String,
    pub(crate) repository_root_if_reject: String,
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
    pub(crate) standing_diff: DecisionInboxStandingDiff,
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
    repository: &'a CurrentRepositoryV3,
    repository_root: &'a str,
    proposal_reference: &'a RepositoryObjectRefV1,
    proposal: &'a ProposalV1,
    claim: &'a ClaimRecordV1,
    submission: &'a SubmissionV1,
    verifications: &'a [(String, VerificationRecordV1)],
    authority_heads: &'a DecisionInboxAuthorityHeads,
}

fn acceptance_blockers(
    submission: &SubmissionV1,
    records: &[(String, VerificationRecordV1)],
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
        let satisfied = records.iter().any(|(_, record)| {
            record.scope.property == *requirement
                && record.outcome == "pass"
                && record.verifier != submission.provenance.producer
                && record
                    .independence
                    .declared_independent_of
                    .contains(&submission.provenance.producer)
        });
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
    blockers.sort();
    blockers.dedup();
    blockers
}

fn accepted_subset(
    repository: &CurrentRepositoryV3,
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

    let blockers = acceptance_blockers(submission, records);
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
    let verification_set_root = verification_set_root(records)?;
    let verification_records = records
        .iter()
        .map(|(root, record)| DecisionInboxVerification {
            verification_record_id: record.verification_record_id.clone(),
            verification_record_root: root.clone(),
            outcome: record.outcome.clone(),
            property: record.scope.property.clone(),
            verifier: record.verifier.clone(),
            independent_of_producer: record.verifier != submission.provenance.producer
                && record
                    .independence
                    .declared_independent_of
                    .contains(&submission.provenance.producer),
            does_not_establish: record.scope.does_not_establish.clone(),
        })
        .collect::<Vec<_>>();
    let acceptance = if blockers.is_empty() {
        "ready"
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
            acceptance: acceptance.into(),
            rejection: "ready".into(),
            blockers,
        },
        standing_diff: DecisionInboxStandingDiff {
            transition,
            target_claim_id,
            accepted_before: accepted_subset(inputs.repository, &affected_claim_ids),
            accepted_if_accept: accepted_subset(&accepted, &affected_claim_ids),
            accepted_if_reject: accepted_subset(&rejected, &affected_claim_ids),
            repository_root_if_accept: accepted.canonical_root()?,
            repository_root_if_reject: rejected.canonical_root()?,
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
    let decisions =
        crate::current_repository::load_current_proposal_decisions(frontier, &repository)?;
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
        if decisions.contains_key(&proposal_reference.id) {
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
        entries.push(derive_entry(EntryInputs {
            repository: &repository,
            repository_root: &repository_root,
            proposal_reference,
            proposal: &proposal,
            claim: &claim,
            submission: &submission,
            verifications: &verifications,
            authority_heads: &authority_heads,
        })?);
    }
    entries.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.proposal_id.cmp(&right.proposal_id))
    });
    let mut projection = DecisionInboxProjection {
        schema: PROJECTION_SCHEMA.into(),
        frontier_id: repository.frontier_id,
        repository_root,
        order: "created_at_asc_then_proposal_id".into(),
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
        println!(
            "\n{} · {} · {}",
            entry.readiness.acceptance, entry.proposal_id, entry.proposal_action
        );
        println!("  {}", crate::cli::safe_text::inline(&entry.assertion));
        println!("  Change: {}", entry.standing_diff.transition);
        println!(
            "  Evidence: {} Verification Record{} · {} blocker{}",
            entry.verification_records.len(),
            if entry.verification_records.len() == 1 {
                ""
            } else {
                "s"
            },
            entry.readiness.blockers.len(),
            if entry.readiness.blockers.len() == 1 {
                ""
            } else {
                "s"
            }
        );
        println!("  Entry: {}", entry.entry_root);
        println!("  Inspect: vela review show . {} --json", entry.proposal_id);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ed25519_dalek::SigningKey;
    use vela_protocol::claim_record::{ClaimAssertion, ClaimRelation, ClaimSource};
    use vela_protocol::current_repository::{
        CURRENT_REPOSITORY_SCHEMA_V3, ClaimStandingRefV1, RepositoryObjectRefV1,
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
    ) -> CurrentRepositoryV3 {
        let proposal_root = proposal.canonical_root().unwrap();
        let submission_root = submission.canonical_root().unwrap();
        let verification_root = verification.canonical_root().unwrap();
        CurrentRepositoryV3 {
            schema: CURRENT_REPOSITORY_SCHEMA_V3.into(),
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
            submissions: vec![RepositoryObjectRefV1 {
                schema: vela_protocol::submission_v1::SUBMISSION_V1_SCHEMA.into(),
                id: submission.submission_id.clone(),
                root: submission_root.clone(),
                path: format!(
                    "records/submissions/sha256/{}.json",
                    submission_root.trim_start_matches("sha256:")
                ),
            }],
            registrations: Vec::new(),
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
        repository: &CurrentRepositoryV3,
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

        assert_eq!(entry.readiness.acceptance, "ready");
        assert!(entry.readiness.blockers.is_empty());
        assert!(
            crate::current_repository_decision::require_acceptance_evidence(
                &submission,
                &[(verification.canonical_root().unwrap(), verification.clone())],
            )
            .is_ok()
        );
        assert!(entry.standing_diff.accepted_before.is_empty());
        assert_eq!(
            entry.standing_diff.accepted_if_accept,
            vec![AcceptedStanding {
                claim_id: subject.claim_id.clone(),
                claim_root: subject.canonical_root().unwrap(),
            }]
        );
        assert!(entry.standing_diff.accepted_if_reject.is_empty());
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

        assert_eq!(entry.readiness.acceptance, "blocked");
        assert_eq!(entry.readiness.rejection, "ready");
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
    fn correction_diff_replaces_only_the_exact_predecessor() {
        let original = claim("Original bounded result.", 1, Vec::new());
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
            vec![claim_standing(&original, "accepted")],
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

        assert_eq!(entry.standing_diff.target_claim_id, original.claim_id);
        assert_eq!(
            entry.standing_diff.accepted_before[0].claim_id,
            original.claim_id
        );
        assert_eq!(
            entry.standing_diff.accepted_if_accept[0].claim_id,
            correction.claim_id
        );
        assert_eq!(
            entry.standing_diff.accepted_if_reject[0].claim_id,
            original.claim_id
        );
        assert_ne!(
            entry.standing_diff.repository_root_if_accept,
            entry.standing_diff.repository_root_if_reject
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
