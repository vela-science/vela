//! Current-only Verification Record intake for Profile v2 repositories.
//!
//! Verification remains scoped authenticated evidence. This writer retains one
//! exact Verification Record and advances the current repository without
//! changing authority or Claim standing.

use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde_json::json;
use sha2::{Digest, Sha256};
use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
use vela_protocol::proposal_v1::ProposalV1;
use vela_protocol::repository::{RepositoryObjectRefV1, RepositoryV4};
use vela_protocol::submission_v1::SubmissionV1;
use vela_protocol::verification_record::{
    IndependenceDisclosure, VerificationMethod, VerificationRecordDraft, VerificationRecordV1,
    VerificationScope, VerificationSubject,
};

use crate::authority_transaction::AuthorityObjectDraft;
use crate::config::git_publish::{
    PublicationOutcome, PublicationState, PublishOptions, exact_publication_preflight,
    publish_exact_delta,
};
use crate::repository_ops::{VerificationImportOutcome, publication_delta};
use crate::repository_txn::{ContentDigest, InputBinding, WriteClass};

const METHOD_MANIFEST_MAX_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct VerificationRecordRequest {
    pub(crate) proposal_id: String,
    pub(crate) profile: String,
    pub(crate) method_path: PathBuf,
    pub(crate) property: Option<String>,
    pub(crate) complementary: bool,
    pub(crate) outcome: String,
    pub(crate) does_not_establish: Vec<String>,
    pub(crate) independent_of: Vec<String>,
    pub(crate) shared_dependencies: Vec<String>,
    pub(crate) actor: String,
}

fn ensure_pending_proposal(
    frontier: &Path,
    repository: &RepositoryV4,
    proposal_id: &str,
) -> Result<(), String> {
    let standings = crate::repository::load_current_proposal_standings(frontier, repository)?;
    ensure_pending_standing(proposal_id, standings.get(proposal_id).map(String::as_str))
}

fn ensure_pending_standing(proposal_id: &str, standing: Option<&str>) -> Result<(), String> {
    if let Some(standing) = standing {
        return Err(format!(
            "Verification Record Proposal {proposal_id} is {}, not pending_review",
            standing
        ));
    }
    Ok(())
}

struct ProposalPackage {
    proposal: ProposalV1,
    proposal_root: String,
    submission: SubmissionV1,
}

fn load_current_proposal_package(
    frontier: &Path,
    repository: &RepositoryV4,
    proposal_id: &str,
) -> Result<ProposalPackage, String> {
    let proposal_reference = repository
        .proposals
        .iter()
        .find(|reference| reference.id == proposal_id)
        .ok_or_else(|| {
            format!(
                "Verification Record Proposal {proposal_id} is not pending in the current repository"
            )
        })?;
    let proposal = read_exact_object(
        frontier,
        proposal_reference,
        ProposalV1::parse,
        ProposalV1::canonical_bytes,
    )?;
    let proposal_root = proposal.canonical_root()?;
    if proposal.proposal_id != proposal_reference.id || proposal_root != proposal_reference.root {
        return Err(format!(
            "current Proposal {proposal_id} differs from its exact repository reference"
        ));
    }
    let submission_reference = repository
        .submissions
        .iter()
        .find(|reference| {
            reference.id == proposal.producer_package.id
                && reference.root == proposal.producer_package.root
                && reference.path == proposal.producer_package.path
        })
        .ok_or_else(|| {
            format!(
                "Verification Record Proposal {proposal_id} does not bind one exact current Submission"
            )
        })?;
    let submission = read_exact_object(
        frontier,
        submission_reference,
        SubmissionV1::parse,
        SubmissionV1::canonical_bytes,
    )?;
    if submission.submission_id != submission_reference.id
        || submission.canonical_root()? != submission_reference.root
    {
        return Err("stored Submission identity differs from the current repository".into());
    }
    Ok(ProposalPackage {
        proposal,
        proposal_root,
        submission,
    })
}

fn current_subject_for_package(
    repository: &RepositoryV4,
    package: &ProposalPackage,
) -> Result<VerificationSubject, String> {
    let mut artifact_ids = Vec::with_capacity(package.submission.artifacts.len());
    for artifact in &package.submission.artifacts {
        let artifact_id = artifact
            .digest
            .strip_prefix("sha256:")
            .ok_or_else(|| {
                "current Submission artifact digest is not a full sha256 identity".to_string()
            })?
            .to_string();
        let retained_path = format!("records/artifacts/sha256/{artifact_id}");
        let exact = repository.artifacts.iter().any(|reference| {
            reference.schema == "content-addressed-artifact"
                && reference.id == artifact_id
                && reference.root == artifact.digest
                && reference.path == retained_path
        });
        if !exact {
            return Err(format!(
                "current Submission Artifact {} differs from its exact repository reference",
                artifact.digest
            ));
        }
        artifact_ids.push(artifact_id);
    }

    Ok(VerificationSubject {
        claim_id: package.proposal.subject.id.clone(),
        artifact_ids,
        submission_id: package.submission.submission_id.clone(),
        submission_root: package.proposal.producer_package.root.clone(),
        proposal_id: package.proposal.proposal_id.clone(),
    })
}

fn method_manifest_binding(
    frontier: &Path,
    method_path: &Path,
) -> Result<(String, String), String> {
    let implementation = method_path
        .components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .map(ToString::to_string)
                .ok_or_else(|| "Verification method path must be UTF-8".to_string()),
            _ => Err(
                "Verification method path must be normalized and repository-relative".to_string(),
            ),
        })
        .collect::<Result<Vec<_>, _>>()?
        .join("/");
    if implementation.trim().is_empty()
        || implementation != implementation.trim()
        || implementation.chars().any(char::is_control)
    {
        return Err("Verification method path must be normalized printable text".into());
    }
    let bytes = crate::bounded_file::read_bounded_frontier_file(
        frontier,
        method_path,
        METHOD_MANIFEST_MAX_BYTES,
        "Verification method manifest",
    )
    .map_err(|error| error.to_string())?;
    if bytes.is_empty() {
        return Err("Verification method manifest must not be empty".into());
    }
    let tracked = vela_edge::git::output(
        frontier,
        &["ls-files", "--error-unmatch", "--", &implementation],
    )?;
    if !tracked.status.success() {
        return Err(
            "Verification method manifest must be retained in the current Git commit".into(),
        );
    }
    let dirt = vela_edge::git::text(
        frontier,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=no",
            "--",
            &implementation,
        ],
    )?;
    if !dirt.is_empty() {
        return Err(
            "Verification method manifest differs from the retained current Git bytes".into(),
        );
    }
    Ok((
        implementation,
        format!("sha256:{}", hex::encode(Sha256::digest(&bytes))),
    ))
}

fn resolve_property(
    requested: Option<String>,
    complementary: bool,
    requirements: &[String],
) -> Result<String, String> {
    match (requested, complementary) {
        (None, false) if requirements.len() == 1 => Ok(requirements[0].clone()),
        (None, false) if requirements.is_empty() => Err(
            "--property is required because this Submission has no registered verification requirement"
                .into(),
        ),
        (None, false) => Err(format!(
            "--property is required because this Submission has {} registered verification requirements; use one exact requirement",
            requirements.len()
        )),
        (None, true) => Err("--complementary requires --property".into()),
        (Some(property), false) if requirements.contains(&property) => Ok(property),
        (Some(property), false) => Err(format!(
            "Verification property {property:?} does not exactly match a registered requirement; omit --property when there is one requirement, use one exact requirement, or add --complementary for an explicitly complementary observation"
        )),
        (Some(property), true) if requirements.contains(&property) => Err(
            "--complementary cannot label a property that exactly satisfies a registered requirement"
                .into(),
        ),
        (Some(property), true) => Ok(property),
    }
}

fn matches_request(
    record: &VerificationRecordV1,
    subject: &VerificationSubject,
    method: &VerificationMethod,
    scope: &VerificationScope,
    outcome: &str,
    verifier: &str,
    independence: &IndependenceDisclosure,
) -> bool {
    record.subject == *subject
        && record.method == *method
        && record.scope == *scope
        && record.outcome == outcome
        && record.verifier == verifier
        && record.independence == *independence
        && record.output_artifact_ids.is_empty()
}

fn existing_semantic_record(
    frontier: &Path,
    repository: &RepositoryV4,
    subject: &VerificationSubject,
    method: &VerificationMethod,
    scope: &VerificationScope,
    outcome: &str,
    verifier: &str,
    independence: &IndependenceDisclosure,
) -> Result<Option<VerificationRecordV1>, String> {
    for reference in &repository.verifications {
        let record = read_exact_object(
            frontier,
            reference,
            VerificationRecordV1::parse,
            VerificationRecordV1::canonical_bytes,
        )?;
        if matches_request(
            &record,
            subject,
            method,
            scope,
            outcome,
            verifier,
            independence,
        ) {
            return Ok(Some(record));
        }
    }
    Ok(None)
}

pub(crate) fn author_record(
    frontier: &Path,
    request: VerificationRecordRequest,
) -> Result<VerificationRecordV1, String> {
    let actor = request.actor.trim();
    if actor != request.actor
        || !(actor.starts_with("agent:")
            || actor.starts_with("ci:")
            || actor.starts_with("verifier:"))
    {
        return Err(
            "verification record author must be an exact agent:, ci:, or verifier: identity".into(),
        );
    }

    // Complete every repository and method preflight before the local agent
    // key resolver is allowed to mint or load a signer.
    let repository = crate::repository::verify_current_repository_at(frontier, true)?;
    ensure_pending_proposal(frontier, &repository, &request.proposal_id)?;
    let package = load_current_proposal_package(frontier, &repository, &request.proposal_id)?;
    let property = resolve_property(
        request.property,
        request.complementary,
        &package.submission.verification_requirements,
    )?;
    let subject = current_subject_for_package(&repository, &package)?;
    let (implementation, environment_root) =
        method_manifest_binding(frontier, &request.method_path)?;
    let method = VerificationMethod {
        profile: request.profile,
        implementation,
        environment_root,
    };
    let scope = VerificationScope {
        property,
        does_not_establish: request.does_not_establish,
    };
    let independence = IndependenceDisclosure {
        declared_independent_of: request.independent_of,
        shared_dependencies: request.shared_dependencies,
    };
    if let Some(record) = existing_semantic_record(
        frontier,
        &repository,
        &subject,
        &method,
        &scope,
        &request.outcome,
        actor,
        &independence,
    )? {
        return Ok(record);
    }

    let observed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let key = vela_edge::agent_identity::agent_signing_key(actor)?;
    let identity = IdentityBinding::build(
        IdentityBindingDraft {
            actor_id: actor.into(),
            actor_class: ActorClass::Agent,
            created_at: observed_at.clone(),
        },
        &key,
    )?;
    VerificationRecordV1::build(
        VerificationRecordDraft {
            subject,
            method,
            scope,
            outcome: request.outcome,
            verifier: actor.into(),
            independence,
            output_artifact_ids: Vec::new(),
            started_at: observed_at.clone(),
            completed_at: observed_at,
        },
        identity,
        &key,
    )
}

fn read_exact_object<T>(
    frontier: &Path,
    reference: &RepositoryObjectRefV1,
    parse: impl FnOnce(&[u8]) -> Result<T, String>,
    canonical_bytes: impl FnOnce(&T) -> Result<Vec<u8>, String>,
) -> Result<T, String> {
    let bytes = fs::read(frontier.join(&reference.path))
        .map_err(|error| format!("read current object {}: {error}", reference.path))?;
    let object = parse(&bytes)?;
    if canonical_bytes(&object)? != bytes {
        return Err(format!(
            "current object {} is not exact canonical JSON",
            reference.path
        ));
    }
    Ok(object)
}

fn load_subject(
    frontier: &Path,
    repository: &RepositoryV4,
    record: &VerificationRecordV1,
) -> Result<(ProposalV1, String, SubmissionV1), String> {
    let package = load_current_proposal_package(frontier, repository, &record.subject.proposal_id)?;
    if package.proposal.subject.id != record.subject.claim_id
        || package.proposal.producer_package.id != record.subject.submission_id
        || package.proposal.producer_package.root != record.subject.submission_root
    {
        return Err(
            "Verification Record does not bind the current Proposal and producer package".into(),
        );
    }

    for artifact_id in record
        .subject
        .artifact_ids
        .iter()
        .chain(&record.output_artifact_ids)
    {
        if !repository
            .artifacts
            .iter()
            .any(|reference| reference.id == *artifact_id)
        {
            return Err(format!(
                "Verification Record names Artifact {artifact_id} outside the current repository"
            ));
        }
    }
    Ok((package.proposal, package.proposal_root, package.submission))
}

fn existing_outcome(
    frontier: &Path,
    repository: &RepositoryV4,
    record: &VerificationRecordV1,
    record_root: &str,
    operation_id: &str,
) -> Result<Option<VerificationImportOutcome>, String> {
    let Some(reference) = repository
        .verifications
        .iter()
        .find(|reference| reference.id == record.verification_record_id)
    else {
        return Ok(None);
    };
    if reference.root != record_root {
        return Err("Verification Record ID collides with different canonical bytes".into());
    }
    let stored = read_exact_object(
        frontier,
        reference,
        VerificationRecordV1::parse,
        VerificationRecordV1::canonical_bytes,
    )?;
    if stored.verification_record_id != record.verification_record_id
        || stored.canonical_root()? != record_root
    {
        return Err("stored Verification Record differs from the current repository".into());
    }
    Ok(Some(VerificationImportOutcome {
        schema: "vela.verification-import-result.v1",
        operation_id: operation_id.into(),
        verification_record_id: record.verification_record_id.clone(),
        verification_record_root: record_root.into(),
        proposal_id: record.subject.proposal_id.clone(),
        claim_id: record.subject.claim_id.clone(),
        outcome: record.outcome.clone(),
        idempotent: true,
        accepted_event_delta: 0,
        publication: PublicationOutcome {
            state: PublicationState::Uncommitted {
                candidate: None,
                reason: "exact Verification Record is already retained".into(),
            },
        },
    }))
}

pub(crate) fn import(
    frontier: &Path,
    record: &VerificationRecordV1,
    executor: &str,
) -> Result<VerificationImportOutcome, String> {
    import_inner(frontier, record, executor)
}

fn import_inner(
    frontier: &Path,
    record: &VerificationRecordV1,
    executor: &str,
) -> Result<VerificationImportOutcome, String> {
    record.verify()?;
    let executor = executor.trim();
    if executor != record.verifier || executor != record.authentication.identity_binding.actor_id {
        return Err("verification import actor must match the Verification Record verifier".into());
    }

    let repository = crate::repository::verify_current_repository_at(frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let (_proposal, proposal_root, submission) = load_subject(frontier, &repository, record)?;
    let record_bytes = record.canonical_bytes()?;
    let record_root = record.canonical_root()?;
    let request_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&json!({
            "schema": "vela.current-verification-import-request.v1",
            "repository_id": repository.repository_id,
            "origin_id": repository.origin_id,
            "repository_before": repository_root,
            "verification_record_root": record_root,
            "source_attempt": submission.provenance.source_attempt.as_deref(),
        }))?
    );
    let operation_id =
        crate::repository_txn::OperationId::derive("verification-import", request_root.as_bytes());
    if let Some(outcome) = existing_outcome(
        frontier,
        &repository,
        record,
        &record_root,
        operation_id.as_str(),
    )? {
        return Ok(outcome);
    }
    ensure_pending_proposal(frontier, &repository, &record.subject.proposal_id)?;

    let journal_dir = crate::repository_ops::repository_transaction_journal_dir(frontier)?;
    let barrier = crate::repository_txn::RepositoryTxn::acquire_routine_evidence_write_barrier(
        frontier,
        &journal_dir,
    )
    .map_err(|error| error.to_string())?;
    let held_repository = crate::repository::verify_current_repository_at(frontier, true)?;
    if held_repository.canonical_root()? != repository_root {
        return Err(
            "current repository changed while acquiring the verification import barrier".into(),
        );
    }
    ensure_pending_proposal(frontier, &held_repository, &record.subject.proposal_id)?;
    let (_, _, held_submission) = load_subject(frontier, &held_repository, record)?;
    if held_submission.canonical_root()? != submission.canonical_root()? {
        return Err(
            "Verification source Submission changed while acquiring the import barrier".into(),
        );
    }

    let record_path = crate::submission::rooted_path("records/verifications/sha256", &record_root)?;
    let mut next_repository = held_repository.clone();
    crate::submission::add_object_ref(
        &mut next_repository.verifications,
        RepositoryObjectRefV1 {
            schema: record.schema.clone(),
            id: record.verification_record_id.clone(),
            root: record_root.clone(),
            path: record_path.clone(),
        },
    )?;
    next_repository.verify()?;
    let derived_drafts = crate::submission::rebind_target_index(frontier, &next_repository)?;

    let recorded_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut prepared = crate::routine_evidence_transaction::prepare_routine_evidence_transaction(
        barrier,
        frontier,
        &held_repository.repository_id,
        crate::repository_txn::OperationKind::Verification,
        operation_id.clone(),
        &request_root,
        recorded_at,
        {
            vec![
                InputBinding {
                    name: "verification_record".into(),
                    digest: ContentDigest::parse(record_root.clone())
                        .map_err(|error| error.to_string())?,
                },
                InputBinding {
                    name: "submission".into(),
                    digest: ContentDigest::parse(record.subject.submission_root.clone())
                        .map_err(|error| error.to_string())?,
                },
                InputBinding {
                    name: "proposal".into(),
                    digest: ContentDigest::parse(proposal_root)
                        .map_err(|error| error.to_string())?,
                },
                InputBinding {
                    name: "current_repository_before".into(),
                    digest: ContentDigest::parse(repository_root)
                        .map_err(|error| error.to_string())?,
                },
            ]
        },
        vec![
            AuthorityObjectDraft {
                path: record_path,
                object_kind: "verification_record".into(),
                class: WriteClass::PublicReview,
                postimage: Some(record_bytes),
            },
            AuthorityObjectDraft {
                path: ".vela/repository.json".into(),
                object_kind: "repository_manifest".into(),
                class: WriteClass::CanonicalEvidence,
                postimage: Some(next_repository.canonical_bytes()?),
            },
        ],
        derived_drafts,
    )?;

    let precommit = (|| {
        let public = prepared
            .resolved_public_writes()
            .map_err(|error| error.to_string())?;
        let delta_root = prepared.canonical_delta_root().to_string();
        let publish_options = PublishOptions::local();
        let delta = publication_delta(frontier, &delta_root, public)?
            .ok_or_else(|| "Verification import had no public Git delta".to_string())?;
        let preflight = exact_publication_preflight(frontier, &delta, &publish_options)
            .map_err(publication_error)?;
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
    crate::repository::verify_current_repository_allow_derived_drift_at(frontier)?;
    let publication = publish_exact_delta(
        frontier,
        "verification import",
        std::slice::from_ref(&record.verification_record_id),
        &delta,
        preflight,
    )
    .map_err(|error| error.to_string())?;
    if matches!(
        publication.state,
        PublicationState::Unchanged { .. } | PublicationState::CommittedLocal { .. }
    ) {
        crate::repository::verify_current_repository_at(frontier, true).map_err(|error| {
            format!(
                "Verification Record was published but strict post-publication verification \
                     failed: {error}; do not retry the import"
            )
        })?;
        if let Err(error) = prepared.retire_completed_recovery_blobs() {
            crate::ui::warn_nonfatal(&format!(
                "Verification import {} was published and verified, but private recovery blob cleanup failed: {error}",
                operation_id.as_str()
            ));
        }
    }
    Ok(VerificationImportOutcome {
        schema: "vela.verification-import-result.v1",
        operation_id: operation_id.as_str().into(),
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

fn publication_error(outcome: PublicationOutcome) -> String {
    match outcome.state {
        PublicationState::Uncommitted { reason, .. } => reason,
        PublicationState::Unchanged { .. } | PublicationState::CommittedLocal { .. } => {
            "unexpected completed publication during preflight".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::proposal_v1::{ProposalProducerPackage, ProposalSubject};
    use vela_protocol::repository::REPOSITORY_SCHEMA_V4;
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

    struct Fixture {
        _directory: TempDir,
        repository: RepositoryV4,
        record: VerificationRecordV1,
        proposal_root: String,
    }

    fn write(frontier: &Path, path: &str, bytes: &[u8]) {
        let absolute = frontier.join(path);
        fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        fs::write(absolute, bytes).unwrap();
    }

    fn fixture() -> Fixture {
        let directory = TempDir::new().unwrap();
        let producer_key = SigningKey::from_bytes(&[51_u8; 32]);
        let producer_identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:producer-fixture".into(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-27T00:00:00Z".into(),
            },
            &producer_key,
        )
        .unwrap();
        let submission = SubmissionV1::build(
            SubmissionDraft {
                claim: SubmissionClaim {
                    assertion: "A bounded witness satisfies the fixture.".into(),
                    claim_type: "computational".into(),
                    conditions: vec!["Fixture domain.".into()],
                },
                artifacts: vec![SubmissionArtifact {
                    kind: "witness".into(),
                    path: "input.json".into(),
                    digest: root('e'),
                }],
                caveats: vec!["Not scientific acceptance.".into()],
                replayability: "exact".into(),
                producer_checks: Vec::new(),
                verification_requirements: vec!["Replay the fixture verifier.".into()],
                requested_change: RequestedChange {
                    kind: "add_claim".into(),
                    target: None,
                },
                provenance: SubmissionProvenance {
                    producer: "agent:producer-fixture".into(),
                    source_system: "fixture".into(),
                    source_attempt: None,
                    source_run: Some("run_fixture".into()),
                    emitted_at: "2026-07-27T00:00:00Z".into(),
                },
                execution_binding: None,
            },
            producer_identity,
            &producer_key,
        )
        .unwrap();
        let submission_root = submission.canonical_root().unwrap();
        let submission_path =
            crate::submission::rooted_path("records/submissions/sha256", &submission_root).unwrap();
        let claim_id = format!("vcl_{}", "a".repeat(64));
        let proposal = ProposalV1::build(
            "claim.add".into(),
            ProposalSubject {
                kind: "claim".into(),
                id: claim_id.clone(),
                root: root('b'),
            },
            "agent:producer-fixture".into(),
            "2026-07-27T00:00:01Z".into(),
            "Fixture proposal.".into(),
            ProposalProducerPackage {
                kind: "submission_v1".into(),
                id: submission.submission_id.clone(),
                root: submission_root.clone(),
                path: submission_path.clone(),
            },
            Vec::new(),
        )
        .unwrap();
        let proposal_root = proposal.canonical_root().unwrap();
        let proposal_path =
            crate::submission::rooted_path("records/proposals/sha256", &proposal_root).unwrap();
        write(
            directory.path(),
            &submission_path,
            &submission.canonical_bytes().unwrap(),
        );
        write(
            directory.path(),
            &proposal_path,
            &proposal.canonical_bytes().unwrap(),
        );

        let verifier_key = SigningKey::from_bytes(&[52_u8; 32]);
        let verifier_identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "service:verifier-fixture".into(),
                actor_class: ActorClass::Org,
                created_at: "2026-07-27T00:00:00Z".into(),
            },
            &verifier_key,
        )
        .unwrap();
        let record = VerificationRecordV1::build(
            VerificationRecordDraft {
                subject: VerificationSubject {
                    claim_id,
                    artifact_ids: vec!["e".repeat(64)],
                    submission_id: submission.submission_id.clone(),
                    submission_root: submission_root.clone(),
                    proposal_id: proposal.proposal_id.clone(),
                },
                method: VerificationMethod {
                    profile: "fixture-v1".into(),
                    implementation: "fixture-verifier".into(),
                    environment_root: root('f'),
                },
                scope: VerificationScope {
                    property: "The bounded witness passes.".into(),
                    does_not_establish: vec!["Scientific acceptance.".into()],
                },
                outcome: "pass".into(),
                verifier: "service:verifier-fixture".into(),
                independence: IndependenceDisclosure {
                    declared_independent_of: vec!["agent:producer-fixture".into()],
                    shared_dependencies: Vec::new(),
                },
                output_artifact_ids: Vec::new(),
                started_at: "2026-07-27T00:00:02Z".into(),
                completed_at: "2026-07-27T00:00:03Z".into(),
            },
            verifier_identity,
            &verifier_key,
        )
        .unwrap();
        let repository = RepositoryV4 {
            schema: REPOSITORY_SCHEMA_V4.into(),
            repository_id: "vrepo_0123456789abcdef".into(),
            profile_root: root('1'),
            origin_id: "vro_0123456789abcdef".into(),
            origin_root: root('2'),
            accepted_claims: Vec::new(),
            pending_claims: Vec::new(),
            proposals: vec![RepositoryObjectRefV1 {
                schema: proposal.schema.clone(),
                id: proposal.proposal_id,
                root: proposal_root.clone(),
                path: proposal_path,
            }],
            proposal_withdrawals: Vec::new(),
            submissions: vec![RepositoryObjectRefV1 {
                schema: submission.schema,
                id: submission.submission_id,
                root: submission_root,
                path: submission_path,
            }],
            verifications: Vec::new(),
            artifacts: vec![RepositoryObjectRefV1 {
                schema: "content-addressed-artifact".into(),
                id: "e".repeat(64),
                root: root('e'),
                path: format!("records/artifacts/sha256/{}", "e".repeat(64)),
            }],
            authority_keyset_root: root('3'),
            authority_policy_root: root('4'),
        };
        repository.verify().unwrap();
        Fixture {
            _directory: directory,
            repository,
            record,
            proposal_root,
        }
    }

    #[test]
    fn current_verification_binds_exact_proposal_submission_claim_and_artifacts() {
        let fixture = fixture();
        let (proposal, proposal_root, submission) = load_subject(
            fixture._directory.path(),
            &fixture.repository,
            &fixture.record,
        )
        .unwrap();
        assert_eq!(proposal_root, fixture.proposal_root);
        assert_eq!(proposal.subject.id, fixture.record.subject.claim_id);
        assert_eq!(
            submission.canonical_root().unwrap(),
            fixture.record.subject.submission_root
        );
    }

    #[test]
    fn verification_with_an_unregistered_artifact_fails_closed() {
        let mut fixture = fixture();
        fixture.repository.artifacts.clear();
        let error = load_subject(
            fixture._directory.path(),
            &fixture.repository,
            &fixture.record,
        )
        .unwrap_err();
        assert!(error.contains("outside the current repository"), "{error}");
    }

    #[test]
    fn verification_with_a_substituted_proposal_root_fails_closed() {
        let mut fixture = fixture();
        fixture.repository.proposals[0].root = root('9');
        let error = load_subject(
            fixture._directory.path(),
            &fixture.repository,
            &fixture.record,
        )
        .unwrap_err();
        assert!(error.contains("exact repository reference"), "{error}");
    }

    #[test]
    fn exact_retained_verification_is_idempotent_and_non_authoritative() {
        let mut fixture = fixture();
        let record_root = fixture.record.canonical_root().unwrap();
        let path =
            crate::submission::rooted_path("records/verifications/sha256", &record_root).unwrap();
        write(
            fixture._directory.path(),
            &path,
            &fixture.record.canonical_bytes().unwrap(),
        );
        fixture
            .repository
            .verifications
            .push(RepositoryObjectRefV1 {
                schema: fixture.record.schema.clone(),
                id: fixture.record.verification_record_id.clone(),
                root: record_root.clone(),
                path,
            });
        let outcome = existing_outcome(
            fixture._directory.path(),
            &fixture.repository,
            &fixture.record,
            &record_root,
            &format!("vop_{}", "a".repeat(64)),
        )
        .unwrap()
        .unwrap();
        assert!(outcome.idempotent);
        assert_eq!(outcome.accepted_event_delta, 0);
    }

    #[test]
    fn semantic_retry_reuses_the_retained_verification() {
        let fixture = fixture();
        assert!(matches_request(
            &fixture.record,
            &fixture.record.subject,
            &fixture.record.method,
            &fixture.record.scope,
            &fixture.record.outcome,
            &fixture.record.verifier,
            &fixture.record.independence,
        ));

        let mut changed_scope = fixture.record.scope.clone();
        changed_scope.property.push_str(" changed");
        assert!(!matches_request(
            &fixture.record,
            &fixture.record.subject,
            &fixture.record.method,
            &changed_scope,
            &fixture.record.outcome,
            &fixture.record.verifier,
            &fixture.record.independence,
        ));
    }

    #[test]
    fn sole_registered_requirement_is_the_default_verification_property() {
        let requirements = vec!["Replay the exact retained artifact.".to_string()];
        assert_eq!(
            resolve_property(None, false, &requirements).unwrap(),
            requirements[0]
        );
        assert_eq!(
            resolve_property(Some(requirements[0].clone()), false, &requirements).unwrap(),
            requirements[0]
        );
    }

    #[test]
    fn ambiguous_or_complementary_properties_require_explicit_intent() {
        let requirements = vec![
            "Replay the exact retained artifact.".to_string(),
            "Inspect its declared scope.".to_string(),
        ];
        let ambiguous = resolve_property(None, false, &requirements).unwrap_err();
        assert!(ambiguous.contains("2 registered verification requirements"));

        let unmatched = resolve_property(
            Some("Inspect another property.".into()),
            false,
            &requirements,
        )
        .unwrap_err();
        assert!(unmatched.contains("does not exactly match"));

        assert_eq!(
            resolve_property(
                Some("Inspect another property.".into()),
                true,
                &requirements
            )
            .unwrap(),
            "Inspect another property."
        );

        let mislabeled =
            resolve_property(Some(requirements[0].clone()), true, &requirements).unwrap_err();
        assert!(mislabeled.contains("exactly satisfies"));
    }

    #[test]
    fn method_manifest_bytes_bind_the_environment_and_fail_closed() {
        let directory = TempDir::new().unwrap();
        let path = Path::new("verification/method.json");
        let initialized = std::process::Command::new("git")
            .current_dir(directory.path())
            .args(["init", "-q"])
            .status()
            .unwrap();
        assert!(initialized.success());
        fs::create_dir_all(directory.path().join("verification")).unwrap();
        fs::write(
            directory.path().join(path),
            br#"{"schema":"fixture.method.v1","version":1}"#,
        )
        .unwrap();
        let committed = std::process::Command::new("git")
            .current_dir(directory.path())
            .args(["add", "verification/method.json"])
            .status()
            .unwrap();
        assert!(committed.success());
        let committed = std::process::Command::new("git")
            .current_dir(directory.path())
            .args([
                "-c",
                "user.name=Vela Test",
                "-c",
                "user.email=vela@example.invalid",
                "commit",
                "-qm",
                "method v1",
            ])
            .status()
            .unwrap();
        assert!(committed.success());
        let (implementation, first_root) = method_manifest_binding(directory.path(), path).unwrap();
        assert_eq!(implementation, "verification/method.json");

        fs::write(
            directory.path().join(path),
            br#"{"schema":"fixture.method.v1","version":2}"#,
        )
        .unwrap();
        let error = method_manifest_binding(directory.path(), path).unwrap_err();
        assert!(error.contains("differs from the retained"), "{error}");
        let committed = std::process::Command::new("git")
            .current_dir(directory.path())
            .args(["add", "verification/method.json"])
            .status()
            .unwrap();
        assert!(committed.success());
        let committed = std::process::Command::new("git")
            .current_dir(directory.path())
            .args([
                "-c",
                "user.name=Vela Test",
                "-c",
                "user.email=vela@example.invalid",
                "commit",
                "-qm",
                "method v2",
            ])
            .status()
            .unwrap();
        assert!(committed.success());
        let (_, second_root) = method_manifest_binding(directory.path(), path).unwrap();
        assert_ne!(first_root, second_root);

        fs::write(directory.path().join(path), []).unwrap();
        let error = method_manifest_binding(directory.path(), path).unwrap_err();
        assert!(error.contains("must not be empty"), "{error}");
        let error =
            method_manifest_binding(directory.path(), Path::new("verification/../outside.json"))
                .unwrap_err();
        assert!(
            error.contains("normalized and repository-relative"),
            "{error}"
        );
    }

    #[test]
    fn terminal_proposals_are_not_verification_authoring_targets() {
        assert!(ensure_pending_standing("vpr_fixture", None).is_ok());
        for standing in ["accepted", "rejected", "withdrawn"] {
            let error =
                ensure_pending_standing("vpr_fixture", Some(standing)).expect_err("terminal");
            assert!(error.contains(standing), "{error}");
            assert!(error.contains("not pending_review"), "{error}");
        }
    }
}
