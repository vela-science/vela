//! Producer intake for Profile v2 repositories.
//!
//! This path consumes an authenticated Submission, creates the current Claim
//! and Proposal, and advances the repository manifest
//! without changing authority or accepted scientific state.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use chrono::{Datelike, SecondsFormat, Utc};
use serde_json::json;
use vela_protocol::claim_record::{
    ClaimAssertion, ClaimEvidenceRef, ClaimRecordV1, ClaimRelation, ClaimSource,
};
use vela_protocol::proposal_v1::{ProposalProducerPackage, ProposalSubject, ProposalV1};
use vela_protocol::repository::{ClaimStandingRefV1, RepositoryObjectRefV1, RepositoryV4};
use vela_protocol::submission_v1::SubmissionV1;

use crate::authority_transaction::{AuthorityDerivedDraft, AuthorityObjectDraft};
use crate::config::git_publish::{
    PublicationOutcome, PublicationState, PublishOptions, exact_publication_preflight,
    publish_exact_delta,
};
use crate::repository_ops::{
    PreparedSubmissionArtifacts, SubmitOutcome, prepare_submission_artifacts, publication_delta,
    submission_publication_inputs,
};
use crate::repository_txn::{ContentDigest, InputBinding, WriteClass};

pub(crate) fn rooted_path(directory: &str, root: &str) -> Result<String, String> {
    let digest = root
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("{directory} object root is not sha256"))?;
    Ok(format!("{directory}/{digest}.json"))
}

pub(crate) fn add_object_ref(
    references: &mut Vec<RepositoryObjectRefV1>,
    reference: RepositoryObjectRefV1,
) -> Result<(), String> {
    if let Some(existing) = references
        .iter()
        .find(|existing| existing.id == reference.id || existing.root == reference.root)
    {
        if existing == &reference {
            return Ok(());
        }
        return Err(format!(
            "current repository object identity collides at {}",
            reference.id
        ));
    }
    references.push(reference);
    references.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(())
}

fn add_artifact_ref(
    references: &mut Vec<RepositoryObjectRefV1>,
    digest: &str,
    path: &str,
) -> Result<(), String> {
    if let Some(existing) = references.iter().find(|existing| existing.root == digest) {
        if existing.path == path {
            // Artifact bytes and their digest have already been checked by
            // prepare_submission_artifacts. Reuse the current repository
            // reference even when predecessor import retained a historical
            // schema label or raw-hex id.
            return Ok(());
        }
        return Err(format!(
            "current repository Artifact root {digest} collides with path {}",
            existing.path
        ));
    }
    let id = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| "current Submission Artifact digest is not sha256".to_string())?;
    add_object_ref(
        references,
        RepositoryObjectRefV1 {
            schema: "content-addressed-artifact".into(),
            id: id.to_string(),
            root: digest.to_string(),
            path: path.to_string(),
        },
    )
}

fn add_pending_claim(
    repository: &mut RepositoryV4,
    claim: &ClaimRecordV1,
    root: &str,
    path: &str,
) -> Result<(), String> {
    if repository
        .accepted_claims
        .iter()
        .chain(&repository.pending_claims)
        .any(|existing| existing.claim_id == claim.claim_id || existing.claim_root == root)
    {
        return Err(format!(
            "current repository already contains Claim {}",
            claim.claim_id
        ));
    }
    repository.pending_claims.push(ClaimStandingRefV1 {
        claim_id: claim.claim_id.clone(),
        claim_root: root.to_string(),
        standing: "pending_review".into(),
        path: path.to_string(),
    });
    repository
        .pending_claims
        .sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    Ok(())
}

fn load_target_claim(
    repository_path: &Path,
    repository: &RepositoryV4,
    submission: &SubmissionV1,
) -> Result<Option<ClaimRecordV1>, String> {
    let Some(target) = submission.requested_change.target.as_ref() else {
        return Ok(None);
    };
    let reference = repository
        .accepted_claims
        .iter()
        .find(|reference| reference.claim_id == target.claim_id)
        .ok_or_else(|| {
            format!(
                "requested Claim {} is not accepted in the current repository",
                target.claim_id
            )
        })?;
    if reference.claim_root != target.claim_root {
        return Err("requested Claim root differs from current accepted state".into());
    }
    let bytes = fs::read(repository_path.join(&reference.path))
        .map_err(|error| format!("read requested Claim: {error}"))?;
    let claim = ClaimRecordV1::parse(&bytes)?;
    if claim.canonical_bytes()? != bytes
        || claim.claim_id != reference.claim_id
        || claim.canonical_root()? != reference.claim_root
    {
        return Err("requested Claim bytes do not match current repository state".into());
    }
    Ok(Some(claim))
}

#[derive(Debug)]
struct ProposedChange {
    action: String,
    subject: ProposalSubject,
    claim: Option<ClaimRecordV1>,
}

fn proposed_change(
    repository_path: &Path,
    repository: &RepositoryV4,
    submission: &SubmissionV1,
) -> Result<ProposedChange, String> {
    let target = load_target_claim(repository_path, repository, submission)?;
    if submission.requested_change.kind == "retract_claim" {
        let target =
            target.ok_or_else(|| "retract_claim requires an accepted Claim".to_string())?;
        let target_root = target.canonical_root()?;
        return Ok(ProposedChange {
            action: "claim.withdraw".into(),
            subject: ProposalSubject {
                kind: "claim".into(),
                id: target.claim_id,
                root: target_root,
            },
            claim: None,
        });
    }

    let mut conditions = submission.claim.conditions.clone();
    conditions.extend(
        submission
            .caveats
            .iter()
            .map(|caveat| format!("Caveat: {caveat}")),
    );
    let evidence = submission
        .artifacts
        .iter()
        .map(|artifact| {
            let digest = artifact
                .digest
                .strip_prefix("sha256:")
                .expect("verified Submission artifact digest is sha256");
            ClaimEvidenceRef {
                relation: "supports".into(),
                artifact_id: None,
                artifact_root: artifact.digest.clone(),
                artifact_path: Some(format!("records/artifacts/sha256/{digest}")),
            }
        })
        .collect::<Vec<_>>();
    let emitted_at = chrono::DateTime::parse_from_rfc3339(&submission.provenance.emitted_at)
        .map_err(|error| format!("Submission emitted_at: {error}"))?;
    let (revision, relations, action) =
        match (submission.requested_change.kind.as_str(), target.as_ref()) {
            ("add_claim", None) => (1, Vec::new(), "claim.add"),
            ("correct_claim", Some(target)) => (
                target.revision.saturating_add(1),
                vec![ClaimRelation {
                    kind: "corrects".into(),
                    target_claim_id: target.claim_id.clone(),
                }],
                "claim.revise",
            ),
            ("supersede_claim", Some(target)) => (
                target.revision.saturating_add(1),
                vec![ClaimRelation {
                    kind: "supersedes".into(),
                    target_claim_id: target.claim_id.clone(),
                }],
                "claim.revise",
            ),
            (kind, _) => {
                return Err(format!(
                    "Submission requested change {kind:?} is inconsistent with its target"
                ));
            }
        };
    let claim = ClaimRecordV1::build(
        revision,
        ClaimAssertion {
            text: submission.claim.assertion.clone(),
            kind: submission.claim.claim_type.clone(),
        },
        conditions,
        evidence,
        vec![ClaimSource {
            kind: "submission".into(),
            title: format!("Authenticated Submission {}", submission.submission_id),
            locator: None,
            authors: vec![submission.provenance.producer.clone()],
            year: Some(emitted_at.year()),
        }],
        relations,
        submission.provenance.emitted_at.clone(),
        BTreeMap::new(),
    )?;
    let root = claim.canonical_root()?;
    Ok(ProposedChange {
        action: action.into(),
        subject: ProposalSubject {
            kind: "claim".into(),
            id: claim.claim_id.clone(),
            root,
        },
        claim: Some(claim),
    })
}

pub(crate) fn rebind_target_index(
    repository_path: &Path,
    repository: &RepositoryV4,
) -> Result<Vec<AuthorityDerivedDraft>, String> {
    let path = repository_path.join("targets.json");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(&path).map_err(|error| format!("read current Target Index: {error}"))?;
    let mut index: vela_edge::target_index::TargetIndexV5 = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse Target Index v5: {error}"))?;
    index.validate()?;
    if index.canonical_bytes()? != bytes || index.repository.origin_id != repository.origin_id {
        return Err("current Target Index is not an exact canonical origin member".into());
    }
    if index
        .inputs
        .entries
        .iter()
        .any(|entry| entry.path == ".vela/repository.json")
    {
        return Err(
            "target_index_invalid_path: current Target Index input \".vela/repository.json\" duplicates the mutable repository binding; regenerate targets.json without it"
                .into(),
        );
    }
    index.repository.repository_root = repository.canonical_root()?;
    index.index_root = index.computed_index_root()?;
    Ok(vec![AuthorityDerivedDraft {
        path: "targets.json".into(),
        postimage: Some(index.canonical_bytes()?),
    }])
}

fn existing_outcome(
    repository_path: &Path,
    repository: &RepositoryV4,
    submission: &SubmissionV1,
    submission_root: &str,
) -> Result<Option<SubmitOutcome>, String> {
    let Some(existing) = repository
        .submissions
        .iter()
        .find(|reference| reference.id == submission.submission_id)
    else {
        return Ok(None);
    };
    if existing.root != submission_root {
        return Err("Submission ID collides with different canonical bytes".into());
    }
    let mut matching = Vec::new();
    for reference in &repository.proposals {
        let bytes = fs::read(repository_path.join(&reference.path))
            .map_err(|error| format!("read existing Proposal {}: {error}", reference.path))?;
        let proposal = ProposalV1::parse(&bytes)?;
        if proposal.producer_package.id == submission.submission_id
            && proposal.producer_package.root == submission_root
            && proposal.producer_package.path == existing.path
        {
            matching.push((reference, proposal));
        }
    }
    let [proposal_reference] = matching.as_slice() else {
        return Err(format!(
            "retained Submission {} must have exactly one Proposal; found {}",
            submission.submission_id,
            matching.len()
        ));
    };
    let (_, proposal) = proposal_reference;
    let request_root = submit_request_root(repository, submission_root)?;
    let operation_id =
        crate::repository_txn::OperationId::derive("submit", request_root.as_bytes());
    Ok(Some(SubmitOutcome {
        schema: "vela.submit-result.v1",
        operation_id: operation_id.as_str().into(),
        submission_id: submission.submission_id.clone(),
        submission_root: submission_root.to_string(),
        proposal_id: proposal.proposal_id.clone(),
        claim_id: proposal.subject.id.clone(),
        route: "pending_review",
        accepted_event_count_before: 0,
        accepted_event_count_after: 0,
        accepted_event_delta: 0,
        accepted_state_changed: false,
        publication: PublicationOutcome {
            state: PublicationState::Uncommitted {
                candidate: None,
                reason: "Submission is already retained in the current repository".into(),
            },
        },
    }))
}

fn submit_request_root(repository: &RepositoryV4, submission_root: &str) -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&json!({
            "schema": "vela.submit-request.v2",
            "repository_id": repository.repository_id,
            "origin_id": repository.origin_id,
            "submission_root": submission_root,
        }))?
    ))
}

fn require_unique_source_run(
    repository_path: &Path,
    repository: &RepositoryV4,
    submission: &SubmissionV1,
) -> Result<(), String> {
    let Some(source_run) = submission.provenance.source_run.as_deref() else {
        return Ok(());
    };
    for reference in &repository.submissions {
        if reference.id == submission.submission_id {
            continue;
        }
        let bytes = fs::read(repository_path.join(&reference.path))
            .map_err(|error| format!("read retained Submission for Run uniqueness: {error}"))?;
        let existing = SubmissionV1::parse(&bytes)?;
        if existing.provenance.producer == submission.provenance.producer
            && existing.provenance.source_system == submission.provenance.source_system
            && existing.provenance.source_run.as_deref() == Some(source_run)
        {
            return Err(format!(
                "Run {source_run} from producer {} in source system {} is already bound to retained Submission {}",
                submission.provenance.producer,
                submission.provenance.source_system,
                existing.submission_id
            ));
        }
    }
    Ok(())
}

pub(crate) fn submit(
    repository_path: &Path,
    submission: &SubmissionV1,
    executor: &str,
    bundle_root: Option<&Path>,
) -> Result<SubmitOutcome, String> {
    submit_inner(repository_path, submission, executor, bundle_root)
}

fn submit_inner(
    repository_path: &Path,
    submission: &SubmissionV1,
    executor: &str,
    bundle_root: Option<&Path>,
) -> Result<SubmitOutcome, String> {
    submission.verify()?;
    let executor = executor.trim();
    if executor != submission.provenance.producer
        || executor != submission.authentication.identity_binding.actor_id
    {
        return Err("submit actor must match the Submission producer identity".into());
    }
    let repository = crate::repository::verify_repository_at(repository_path, true)?;
    let repository_root = repository.canonical_root()?;
    let submission_root = submission.canonical_root()?;
    if let Some(outcome) =
        existing_outcome(repository_path, &repository, submission, &submission_root)?
    {
        return Ok(outcome);
    }

    let journal_dir = crate::repository_ops::repository_transaction_journal_dir(repository_path)?;
    let barrier = crate::repository_txn::RepositoryTxn::acquire_routine_evidence_write_barrier(
        repository_path,
        &journal_dir,
    )
    .map_err(|error| error.to_string())?;
    let held_repository = crate::repository::verify_repository_at(repository_path, true)?;
    if held_repository.canonical_root()? != repository_root {
        return Err("current repository changed while acquiring the submit barrier".into());
    }
    require_unique_source_run(repository_path, &held_repository, submission)?;
    let fixed_time = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let PreparedSubmissionArtifacts {
        writes: artifact_writes,
        mut read_set,
    } = prepare_submission_artifacts(repository_path, submission, bundle_root)?;
    read_set.push(InputBinding {
        name: "submission".into(),
        digest: ContentDigest::parse(submission_root.clone()).map_err(|error| error.to_string())?,
    });
    read_set.push(InputBinding {
        name: "current_repository_before".into(),
        digest: ContentDigest::parse(repository_root.clone()).map_err(|error| error.to_string())?,
    });

    let change = proposed_change(repository_path, &held_repository, submission)?;
    let mut next_repository = held_repository.clone();
    let mut object_drafts = Vec::new();
    if let Some(claim) = &change.claim {
        let claim_root = claim.canonical_root()?;
        let claim_path = rooted_path("records/claims/sha256", &claim_root)?;
        add_pending_claim(&mut next_repository, claim, &claim_root, &claim_path)?;
        object_drafts.push(AuthorityObjectDraft {
            path: claim_path,
            object_kind: "claim_record".into(),
            class: WriteClass::CanonicalEvidence,
            postimage: Some(claim.canonical_bytes()?),
        });
    }
    let submission_path = rooted_path("records/submissions/sha256", &submission_root)?;
    let proposal = ProposalV1::build(
        change.action,
        change.subject,
        executor.into(),
        fixed_time.clone(),
        format!(
            "Retain authenticated Submission {} for independent verification and authorized review.",
            submission.submission_id
        ),
        ProposalProducerPackage {
            kind: "submission_v1".into(),
            id: submission.submission_id.clone(),
            root: submission_root.clone(),
            path: submission_path.clone(),
        },
        submission.caveats.clone(),
    )?;
    let proposal_root = proposal.canonical_root()?;
    let proposal_path = rooted_path("records/proposals/sha256", &proposal_root)?;
    add_object_ref(
        &mut next_repository.proposals,
        RepositoryObjectRefV1 {
            schema: proposal.schema.clone(),
            id: proposal.proposal_id.clone(),
            root: proposal_root.clone(),
            path: proposal_path.clone(),
        },
    )?;
    add_object_ref(
        &mut next_repository.submissions,
        RepositoryObjectRefV1 {
            schema: submission.schema.clone(),
            id: submission.submission_id.clone(),
            root: submission_root.clone(),
            path: submission_path.clone(),
        },
    )?;
    for artifact in &submission.artifacts {
        let digest = artifact
            .digest
            .strip_prefix("sha256:")
            .expect("verified Submission artifact digest is sha256");
        add_artifact_ref(
            &mut next_repository.artifacts,
            &artifact.digest,
            &format!("records/artifacts/sha256/{digest}"),
        )?;
    }
    let request_root = submit_request_root(&held_repository, &submission_root)?;
    let operation_id =
        crate::repository_txn::OperationId::derive("submit", request_root.as_bytes());
    next_repository.verify()?;
    let derived_drafts = rebind_target_index(repository_path, &next_repository)?;
    object_drafts.extend([
        AuthorityObjectDraft {
            path: proposal_path,
            object_kind: "proposal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(proposal.canonical_bytes()?),
        },
        AuthorityObjectDraft {
            path: submission_path,
            object_kind: "submission".into(),
            class: WriteClass::PublicReview,
            postimage: Some(submission.canonical_bytes()?),
        },
        AuthorityObjectDraft {
            path: ".vela/repository.json".into(),
            object_kind: "repository_manifest".into(),
            class: WriteClass::CanonicalEvidence,
            postimage: Some(next_repository.canonical_bytes()?),
        },
    ]);
    for write in artifact_writes {
        let (path, class, postimage) = write
            .into_authority_object_parts()
            .map_err(|error| error.to_string())?;
        object_drafts.push(AuthorityObjectDraft {
            path,
            object_kind: "submission_artifact".into(),
            class,
            postimage,
        });
    }

    let mut prepared = crate::routine_evidence_transaction::prepare_routine_evidence_transaction(
        barrier,
        repository_path,
        &held_repository.repository_id,
        crate::repository_txn::OperationKind::Submission,
        operation_id.clone(),
        &request_root,
        fixed_time,
        read_set,
        object_drafts,
        derived_drafts,
    )?;

    let precommit = (|| {
        let public = prepared
            .resolved_public_writes()
            .map_err(|error| error.to_string())?;
        let delta_root = prepared.canonical_delta_root().to_string();
        let publish_options = PublishOptions::local()
            .with_preflight_inputs(submission_publication_inputs(repository_path, submission)?);
        let delta = publication_delta(repository_path, &delta_root, public)?
            .ok_or_else(|| "Submission transaction had no public Git delta".to_string())?;
        let preflight = exact_publication_preflight(repository_path, &delta, &publish_options)
            .map_err(crate::repository_ops::publication_error)?;
        Ok::<_, String>((delta, preflight))
    })();
    let (delta, preflight) = match precommit {
        Ok(value) => value,
        Err(error) => {
            prepared
                .abort_prepared()
                .map_err(|abort| format!("{error}; abort failed: {abort}"))?;
            return Err(error);
        }
    };
    prepared
        .mark_committed()
        .map_err(|error| error.to_string())?;
    prepared.install().map_err(|error| error.to_string())?;
    prepared.complete().map_err(|error| error.to_string())?;
    crate::repository::verify_repository_allow_derived_drift_at(repository_path)?;
    let publication = publish_exact_delta(
        repository_path,
        "submit",
        std::slice::from_ref(&proposal.proposal_id),
        &delta,
        preflight,
    )
    .map_err(|error| error.to_string())?;
    if matches!(
        publication.state,
        PublicationState::Unchanged { .. } | PublicationState::CommittedLocal { .. }
    ) {
        crate::repository::verify_repository_at(repository_path, true).map_err(|error| {
            format!(
                "Submission was published but strict post-publication verification failed: \
                     {error}; do not retry the Submission"
            )
        })?;
        if let Err(error) = prepared.retire_completed_recovery_blobs() {
            crate::ui::warn_nonfatal(&format!(
                "Submission {} was published and verified, but private recovery blob cleanup failed: {error}",
                operation_id.as_str()
            ));
        }
    }
    Ok(SubmitOutcome {
        schema: "vela.submit-result.v1",
        operation_id: operation_id.as_str().into(),
        submission_id: submission.submission_id.clone(),
        submission_root,
        proposal_id: proposal.proposal_id,
        claim_id: proposal.subject.id,
        route: "pending_review",
        accepted_event_count_before: 0,
        accepted_event_count_after: 0,
        accepted_event_delta: 0,
        accepted_state_changed: false,
        publication,
    })
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::repository::REPOSITORY_SCHEMA_V4;
    use vela_protocol::submission_v1::{
        RequestedChange, RequestedChangeTarget, SubmissionArtifact, SubmissionClaim,
        SubmissionDraft, SubmissionProvenance,
    };

    use super::*;

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn repository() -> RepositoryV4 {
        RepositoryV4 {
            schema: REPOSITORY_SCHEMA_V4.into(),
            repository_id: "vrepo_0123456789abcdef0123456789abcdef".into(),
            profile_root: root('a'),
            origin_id: "vro_0123456789abcdef".into(),
            origin_root: root('b'),
            accepted_claims: Vec::new(),
            pending_claims: Vec::new(),
            proposals: Vec::new(),
            proposal_withdrawals: Vec::new(),
            submissions: Vec::new(),
            verifications: Vec::new(),
            artifacts: Vec::new(),
            authority_keyset_root: root('c'),
            authority_policy_root: root('d'),
        }
    }

    fn submission(
        kind: &str,
        target: Option<RequestedChangeTarget>,
        assertion: &str,
    ) -> SubmissionV1 {
        let key = SigningKey::from_bytes(&[41_u8; 32]);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:current-submission-fixture".into(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-27T00:00:00Z".into(),
            },
            &key,
        )
        .unwrap();
        SubmissionV1::build(
            SubmissionDraft {
                claim: SubmissionClaim {
                    assertion: assertion.into(),
                    claim_type: "computational".into(),
                    conditions: vec!["Fixture domain only.".into()],
                },
                artifacts: vec![SubmissionArtifact {
                    kind: "witness".into(),
                    path: "result.json".into(),
                    digest: root('e'),
                }],
                caveats: vec!["Does not establish an unrestricted result.".into()],
                replayability: "exact".into(),
                producer_checks: Vec::new(),
                verification_requirements: vec!["Replay the frozen verifier.".into()],
                requested_change: RequestedChange {
                    kind: kind.into(),
                    target,
                },
                provenance: SubmissionProvenance {
                    producer: "agent:current-submission-fixture".into(),
                    source_system: "fixture".into(),
                    source_attempt: None,
                    source_run: Some("run_fixture".into()),
                    emitted_at: "2026-07-27T00:00:00Z".into(),
                },
                execution_binding: None,
            },
            identity,
            &key,
        )
        .unwrap()
    }

    fn install_accepted_claim(
        repository_path: &Path,
        repository: &mut RepositoryV4,
        claim: &ClaimRecordV1,
    ) -> String {
        let claim_root = claim.canonical_root().unwrap();
        let path = rooted_path("records/claims/sha256", &claim_root).unwrap();
        let absolute = repository_path.join(&path);
        fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        fs::write(&absolute, claim.canonical_bytes().unwrap()).unwrap();
        repository.accepted_claims.push(ClaimStandingRefV1 {
            claim_id: claim.claim_id.clone(),
            claim_root: claim_root.clone(),
            standing: "accepted".into(),
            path,
        });
        claim_root
    }

    fn accepted_claim() -> ClaimRecordV1 {
        ClaimRecordV1::build(
            1,
            ClaimAssertion {
                text: "Original bounded assertion.".into(),
                kind: "computational".into(),
            },
            vec!["Original fixture domain.".into()],
            Vec::new(),
            vec![ClaimSource {
                kind: "fixture".into(),
                title: "Original fixture".into(),
                locator: None,
                authors: vec!["agent:original".into()],
                year: Some(2026),
            }],
            Vec::new(),
            "2026-07-26T00:00:00Z".into(),
            BTreeMap::new(),
        )
        .unwrap()
    }

    #[test]
    fn exact_artifact_root_reuses_predecessor_reference() {
        let digest = root('e');
        let path = format!("records/artifacts/sha256/{}", "e".repeat(64));
        let mut references = vec![RepositoryObjectRefV1 {
            schema: "canopus.verifier-manifest.v1".into(),
            id: "e".repeat(64),
            root: digest.clone(),
            path: path.clone(),
        }];

        add_artifact_ref(&mut references, &digest, &path).unwrap();

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].schema, "canopus.verifier-manifest.v1");
    }

    #[test]
    fn artifact_root_reuse_rejects_a_different_path() {
        let digest = root('e');
        let mut references = vec![RepositoryObjectRefV1 {
            schema: "content-addressed-artifact".into(),
            id: "e".repeat(64),
            root: digest.clone(),
            path: "records/artifacts/sha256/original".into(),
        }];

        let error = add_artifact_ref(
            &mut references,
            &digest,
            "records/artifacts/sha256/different",
        )
        .unwrap_err();

        assert!(error.contains("collides with path"));
    }

    #[test]
    fn canonical_run_binding_blocks_reexport_when_private_progress_was_lost() {
        let repository_path = TempDir::new().unwrap();
        let mut repository = repository();
        let first = submission("add_claim", None, "First bounded assertion.");
        let first_root = first.canonical_root().unwrap();
        let first_path = rooted_path("records/submissions/sha256", &first_root).unwrap();
        let absolute = repository_path.path().join(&first_path);
        fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        fs::write(&absolute, first.canonical_bytes().unwrap()).unwrap();
        repository.submissions.push(RepositoryObjectRefV1 {
            schema: first.schema.clone(),
            id: first.submission_id.clone(),
            root: first_root,
            path: first_path,
        });

        let reexport = submission("add_claim", None, "Re-exported bounded assertion.");
        assert_ne!(first.submission_id, reexport.submission_id);
        let error =
            require_unique_source_run(repository_path.path(), &repository, &reexport).unwrap_err();

        assert!(error.contains("already bound"), "{error}");
        assert!(error.contains(&first.submission_id), "{error}");
    }

    #[test]
    fn run_identity_is_scoped_to_producer_and_source_system() {
        let repository_path = TempDir::new().unwrap();
        let mut repository = repository();
        let first = submission("add_claim", None, "First bounded assertion.");
        let first_root = first.canonical_root().unwrap();
        let first_path = rooted_path("records/submissions/sha256", &first_root).unwrap();
        let absolute = repository_path.path().join(&first_path);
        fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        fs::write(&absolute, first.canonical_bytes().unwrap()).unwrap();
        repository.submissions.push(RepositoryObjectRefV1 {
            schema: first.schema.clone(),
            id: first.submission_id.clone(),
            root: first_root,
            path: first_path,
        });

        let mut different_source = submission("add_claim", None, "Independent bounded assertion.");
        different_source.provenance.source_system = "another-native-runner".into();
        assert!(
            require_unique_source_run(repository_path.path(), &repository, &different_source)
                .is_ok()
        );

        let mut different_producer = submission("add_claim", None, "Another bounded assertion.");
        different_producer.provenance.producer = "agent:another-producer".into();
        assert!(
            require_unique_source_run(repository_path.path(), &repository, &different_producer)
                .is_ok()
        );
    }

    #[test]
    fn add_submission_derives_a_pending_claim_without_accepted_state() {
        let repository_path = TempDir::new().unwrap();
        let mut repository = repository();
        let change = proposed_change(
            repository_path.path(),
            &repository,
            &submission("add_claim", None, "New bounded assertion."),
        )
        .unwrap();
        let claim = change.claim.unwrap();
        let claim_root = claim.canonical_root().unwrap();
        let claim_path = rooted_path("records/claims/sha256", &claim_root).unwrap();
        add_pending_claim(&mut repository, &claim, &claim_root, &claim_path).unwrap();

        assert_eq!(change.action, "claim.add");
        assert!(repository.accepted_claims.is_empty());
        assert_eq!(repository.pending_claims.len(), 1);
        assert_eq!(repository.pending_claims[0].standing, "pending_review");
        assert_eq!(claim.evidence[0].artifact_root, root('e'));
        assert!(
            claim
                .conditions
                .iter()
                .any(|value| value.starts_with("Caveat:"))
        );
    }

    #[test]
    fn correction_binds_the_exact_current_claim_and_increments_revision() {
        let repository_path = TempDir::new().unwrap();
        let mut repository = repository();
        let original = accepted_claim();
        let original_root =
            install_accepted_claim(repository_path.path(), &mut repository, &original);
        let change = proposed_change(
            repository_path.path(),
            &repository,
            &submission(
                "correct_claim",
                Some(RequestedChangeTarget {
                    claim_id: original.claim_id.clone(),
                    claim_root: original_root,
                }),
                "Corrected bounded assertion.",
            ),
        )
        .unwrap();
        let replacement = change.claim.unwrap();

        assert_eq!(change.action, "claim.revise");
        assert_eq!(replacement.revision, 2);
        assert_eq!(replacement.relations[0].kind, "corrects");
        assert_eq!(replacement.relations[0].target_claim_id, original.claim_id);
    }

    #[test]
    fn retraction_targets_the_existing_claim_without_minting_a_replacement() {
        let repository_path = TempDir::new().unwrap();
        let mut repository = repository();
        let original = accepted_claim();
        let original_root =
            install_accepted_claim(repository_path.path(), &mut repository, &original);
        let change = proposed_change(
            repository_path.path(),
            &repository,
            &submission(
                "retract_claim",
                Some(RequestedChangeTarget {
                    claim_id: original.claim_id.clone(),
                    claim_root: original_root.clone(),
                }),
                "Retraction context.",
            ),
        )
        .unwrap();

        assert_eq!(change.action, "claim.withdraw");
        assert!(change.claim.is_none());
        assert_eq!(change.subject.id, original.claim_id);
        assert_eq!(change.subject.root, original_root);
    }

    #[test]
    fn correction_with_a_different_root_fails_closed() {
        let repository_path = TempDir::new().unwrap();
        let mut repository = repository();
        let original = accepted_claim();
        install_accepted_claim(repository_path.path(), &mut repository, &original);
        let error = proposed_change(
            repository_path.path(),
            &repository,
            &submission(
                "correct_claim",
                Some(RequestedChangeTarget {
                    claim_id: original.claim_id,
                    claim_root: root('f'),
                }),
                "Forged correction.",
            ),
        )
        .unwrap_err();
        assert!(
            error.contains("root differs from current accepted state"),
            "{error}"
        );
    }
}
