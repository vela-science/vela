//! Verification Record intake for Repository Profile v1 repositories.
//!
//! Verification remains scoped authenticated evidence. This writer retains one
//! exact Verification Record and advances the current repository without
//! changing authority or Claim standing.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde_json::json;
use vela_protocol::canonical::sha256_root;
use vela_protocol::proposal::ProposalV1;
use vela_protocol::repository::{RepositoryObjectRefV1, RepositoryV4};
use vela_protocol::review_method::{REVIEW_METHOD_V1_SCHEMA, ReviewMethodV1};
use vela_protocol::signer_identity::{ActorClass, SignerIdentityV1};
use vela_protocol::submission::SubmissionRecordV2;
use vela_protocol::verification_record::{
    IndependenceDisclosure, VerificationMethod, VerificationRecordDraft,
    VerificationRecordEnvelopeV2, VerificationScope, VerificationSubject,
};

use crate::authority_transaction::AuthorityObjectDraft;
use crate::config::git_publish::{
    PublicationOutcome, PublicationState, PublishOptions, exact_git_output, publish_exact_delta,
};
use crate::repository_ops::VerificationImportOutcome;
use vela_repository::{ContentDigest, InputBinding, WriteClass};

const METHOD_MANIFEST_MAX_BYTES: u64 = 1024 * 1024;
const RETAINED_ERROR: &str =
    "Verification method manifest must be retained in the current Git commit";
const DIRTY_ERROR: &str =
    "Verification method manifest differs from the retained current Git bytes";
const REPOSITORY_OPERATION_KIND: &str = "verification";

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
    repository_path: &Path,
    repository: &RepositoryV4,
    proposal_id: &str,
) -> Result<(), String> {
    let standings =
        crate::repository::load_current_proposal_standings(repository_path, repository)?;
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
    submission: SubmissionRecordV2,
}

fn load_proposal_package(
    repository_path: &Path,
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
        repository_path,
        proposal_reference,
        ProposalV1::parse,
        ProposalV1::canonical_bytes,
    )?;
    let proposal_root = proposal.canonical_root()?;
    if proposal.id() != proposal_reference.id || proposal_root != proposal_reference.root {
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
        repository_path,
        submission_reference,
        SubmissionRecordV2::parse,
        |value: &SubmissionRecordV2| Ok(value.bytes.clone()),
    )?;
    if submission.id != submission_reference.id
        || submission.root.clone() != submission_reference.root
    {
        return Err("stored Submission identity differs from the current repository".into());
    }
    Ok(ProposalPackage {
        proposal,
        proposal_root,
        submission,
    })
}

fn subject_for_package(
    repository: &RepositoryV4,
    package: &ProposalPackage,
) -> Result<VerificationSubject, String> {
    let mut artifact_ids = Vec::with_capacity(package.submission.submission.artifacts.len());
    for artifact in &package.submission.submission.artifacts {
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
        submission_id: package.submission.id.clone(),
        submission_root: package.proposal.producer_package.root.clone(),
        proposal_id: package.proposal.id(),
        proposal_root: package.proposal.canonical_root()?,
    })
}

fn method_manifest_binding(
    repository_path: &Path,
    method_path: &Path,
) -> Result<(String, String, Option<ReviewMethodV1>), String> {
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
    let bytes = crate::bounded_file::read_bounded_repository_file(
        repository_path,
        method_path,
        METHOD_MANIFEST_MAX_BYTES,
        "Verification method manifest",
    )
    .map_err(|error| error.to_string())?;
    if bytes.is_empty() {
        return Err("Verification method manifest must not be empty".into());
    }
    let git = |args: &[&str]| exact_git_output(repository_path, args);
    let index_mode = git(&["ls-files", "-s", "-z", "--", &implementation])?;
    if !index_mode.status.success() || index_mode.stdout.is_empty() {
        return Err(RETAINED_ERROR.into());
    }
    let index_spec = format!(":./{implementation}");
    let index = git(&["cat-file", "blob", &index_spec])?;
    let head_spec = format!("HEAD:./{implementation}");
    let head = git(&["cat-file", "blob", &head_spec])?;
    let head_mode = git(&["ls-tree", "-z", "HEAD", "--", &implementation])?;
    let index_executable = tracked_blob_executable(&index_mode);
    let modes_match =
        index_executable.is_some() && tracked_blob_executable(&head_mode) == index_executable;
    #[cfg(unix)]
    let worktree_executable = fs::symlink_metadata(repository_path.join(method_path))
        .map_err(|error| format!("inspect Verification method manifest mode: {error}"))?
        .permissions()
        .mode()
        & 0o111
        != 0;
    #[cfg(unix)]
    let modes_match = modes_match && index_executable == Some(worktree_executable);
    if !index.status.success()
        || !head.status.success()
        || !modes_match
        || head.stdout != bytes
        || index.stdout != bytes
    {
        return Err(DIRTY_ERROR.into());
    }
    let review_method = review_method_if_declared(&bytes)?;
    Ok((implementation, sha256_root(&bytes), review_method))
}

fn review_method_if_declared(bytes: &[u8]) -> Result<Option<ReviewMethodV1>, String> {
    let strict = vela_protocol::canonical::from_json_slice_strict::<serde_json::Value>(bytes);
    match strict {
        Ok(value)
            if value.get("schema").and_then(serde_json::Value::as_str)
                == Some(REVIEW_METHOD_V1_SCHEMA) =>
        {
            ReviewMethodV1::parse_canonical(bytes).map(Some)
        }
        Ok(_) => Ok(None),
        Err(error)
            if bytes
                .windows(REVIEW_METHOD_V1_SCHEMA.len())
                .any(|window| window == REVIEW_METHOD_V1_SCHEMA.as_bytes()) =>
        {
            Err(format!("invalid declared Review Method v1: {error}"))
        }
        Err(_) => Ok(None),
    }
}

fn verification_actor_class(actor: &str) -> Result<ActorClass, String> {
    if actor.starts_with("human:") {
        Ok(ActorClass::Human)
    } else if actor.starts_with("org:") {
        Ok(ActorClass::Org)
    } else if actor.starts_with("agent:")
        || actor.starts_with("ci:")
        || actor.starts_with("verifier:")
    {
        Ok(ActorClass::Agent)
    } else {
        Err(
            "verification record author must be an exact human:, org:, agent:, ci:, or verifier: identity"
                .into(),
        )
    }
}

fn ensure_review_method_binding(
    review_method: &ReviewMethodV1,
    method: &VerificationMethod,
    scope: &VerificationScope,
    actor: &str,
) -> Result<(), String> {
    if review_method.profile != method.profile {
        return Err("Review Method profile differs from --profile".into());
    }
    if review_method.property != scope.property {
        return Err("Review Method property differs from the observed property".into());
    }
    if review_method.attested_by_actor_id != actor {
        return Err("Review Method attesting actor differs from --as".into());
    }
    for nonclaim in &review_method.does_not_establish {
        if !scope.does_not_establish.contains(nonclaim) {
            return Err(format!(
                "Verification Record omits Review Method nonclaim {nonclaim:?}"
            ));
        }
    }
    Ok(())
}

fn tracked_blob_executable(output: &std::process::Output) -> Option<bool> {
    let entry = output.stdout.strip_suffix(&[0])?;
    (output.status.success() && !entry.contains(&0)).then_some(())?;
    match entry.get(..7)? {
        b"100644 " => Some(false),
        b"100755 " => Some(true),
        _ => None,
    }
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
    record: &VerificationRecordEnvelopeV2,
    subject: &VerificationSubject,
    method: &VerificationMethod,
    scope: &VerificationScope,
    outcome: &str,
    verifier: &str,
    independence: &IndependenceDisclosure,
) -> bool {
    record.record.subject == *subject
        && record.record.method == *method
        && record.record.scope == *scope
        && record.record.outcome == outcome
        && record.record.verifier() == verifier
        && record.record.independence == *independence
        && record.record.output_artifact_ids.is_empty()
}

fn existing_semantic_record(
    repository_path: &Path,
    repository: &RepositoryV4,
    subject: &VerificationSubject,
    method: &VerificationMethod,
    scope: &VerificationScope,
    outcome: &str,
    verifier: &str,
    independence: &IndependenceDisclosure,
) -> Result<Option<VerificationRecordEnvelopeV2>, String> {
    for reference in &repository.verifications {
        let record = read_exact_object(
            repository_path,
            reference,
            VerificationRecordEnvelopeV2::parse,
            |value: &VerificationRecordEnvelopeV2| Ok(value.bytes.clone()),
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
    repository_path: &Path,
    request: VerificationRecordRequest,
) -> Result<VerificationRecordEnvelopeV2, String> {
    let actor = request.actor.trim();
    if actor != request.actor {
        return Err("verification record author must be an exact trimmed identity".into());
    }
    let actor_class = verification_actor_class(actor)?;

    // Complete every repository and method preflight before the local agent
    // key resolver is allowed to mint or load a signer.
    let repository = crate::repository::verify_repository_at(repository_path, true)?;
    ensure_pending_proposal(repository_path, &repository, &request.proposal_id)?;
    let package = load_proposal_package(repository_path, &repository, &request.proposal_id)?;
    let property = resolve_property(
        request.property,
        request.complementary,
        &package.submission.submission.verification_requirements,
    )?;
    let subject = subject_for_package(&repository, &package)?;
    let (implementation, environment_root, review_method) =
        method_manifest_binding(repository_path, &request.method_path)?;
    let method = VerificationMethod {
        profile: request.profile,
        implementation,
        environment_root,
    };
    let scope = VerificationScope {
        property,
        does_not_establish: request.does_not_establish,
    };
    if let Some(review_method) = &review_method {
        ensure_review_method_binding(review_method, &method, &scope, actor)?;
    }
    let independence = IndependenceDisclosure {
        declared_independent_of: request.independent_of,
        shared_dependencies: request.shared_dependencies,
    };
    if let Some(record) = existing_semantic_record(
        repository_path,
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
    let identity = SignerIdentityV1::new(actor, actor_class, &key, observed_at.clone())?;
    VerificationRecordEnvelopeV2::seal(
        VerificationRecordDraft {
            subject,
            method,
            scope,
            outcome: request.outcome,
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
    repository_path: &Path,
    reference: &RepositoryObjectRefV1,
    parse: impl FnOnce(&[u8]) -> Result<T, String>,
    canonical_bytes: impl FnOnce(&T) -> Result<Vec<u8>, String>,
) -> Result<T, String> {
    let bytes = fs::read(repository_path.join(&reference.path))
        .map_err(|error| format!("read object {}: {error}", reference.path))?;
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
    repository_path: &Path,
    repository: &RepositoryV4,
    record: &VerificationRecordEnvelopeV2,
) -> Result<(ProposalV1, String, SubmissionRecordV2), String> {
    let package = load_proposal_package(
        repository_path,
        repository,
        &record.record.subject.proposal_id,
    )?;
    if package.proposal.subject.id != record.record.subject.claim_id
        || package.proposal.producer_package.id != record.record.subject.submission_id
        || package.proposal.producer_package.root != record.record.subject.submission_root
    {
        return Err(
            "Verification Record does not bind the current Proposal and producer package".into(),
        );
    }

    for artifact_id in record
        .record
        .subject
        .artifact_ids
        .iter()
        .chain(&record.record.output_artifact_ids)
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
    repository_path: &Path,
    repository: &RepositoryV4,
    record: &VerificationRecordEnvelopeV2,
    record_root: &str,
    operation_id: &str,
) -> Result<Option<VerificationImportOutcome>, String> {
    let Some(reference) = repository
        .verifications
        .iter()
        .find(|reference| reference.id == record.id)
    else {
        return Ok(None);
    };
    if reference.root != record_root {
        return Err("Verification Record ID collides with different canonical bytes".into());
    }
    let stored = read_exact_object(
        repository_path,
        reference,
        VerificationRecordEnvelopeV2::parse,
        |value: &VerificationRecordEnvelopeV2| Ok(value.bytes.clone()),
    )?;
    if stored.id != record.id || stored.root.clone() != record_root {
        return Err("stored Verification Record differs from the current repository".into());
    }
    Ok(Some(VerificationImportOutcome {
        schema: "vela.verification-import-result.v1",
        operation_id: operation_id.into(),
        verification_record_id: record.id.clone(),
        verification_record_root: record_root.into(),
        proposal_id: record.record.subject.proposal_id.clone(),
        claim_id: record.record.subject.claim_id.clone(),
        outcome: record.record.outcome.clone(),
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
    repository_path: &Path,
    record: &VerificationRecordEnvelopeV2,
    executor: &str,
) -> Result<VerificationImportOutcome, String> {
    let executor = executor.trim();
    if executor != record.record.verifier() || executor != record.record.identity.actor_id {
        return Err("verification import actor must match the Verification Record verifier".into());
    }

    let repository = crate::repository::verify_repository_at(repository_path, true)?;
    let repository_root = repository.canonical_root()?;
    crate::repository_ops::verify_repository_transaction_barrier_read_only(repository_path)?;
    let (_proposal, proposal_root, submission) =
        load_subject(repository_path, &repository, record)?;
    let record_bytes = record.bytes.clone();
    let record_root = record.root.clone();
    let request_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&json!({
            "schema": "vela.verification-import-request.v1",
            "repository_id": repository.repository_id,
            "origin_id": repository.origin_id,
            "repository_before": repository_root,
            "verification_record_root": record_root,
            "source_run": submission.submission.provenance.source_run.as_deref(),
        }))?
    );
    let operation_id =
        vela_repository::OperationId::derive("verification-import", request_root.as_bytes());
    if let Some(outcome) = existing_outcome(
        repository_path,
        &repository,
        record,
        &record_root,
        operation_id.as_str(),
    )? {
        return Ok(outcome);
    }
    ensure_pending_proposal(
        repository_path,
        &repository,
        &record.record.subject.proposal_id,
    )?;

    let journal_dir = crate::repository_ops::repository_transaction_journal_dir(repository_path)?;
    let barrier = crate::repository_write_policy::acquire_routine_evidence_write_barrier(
        repository_path,
        &journal_dir,
    )
    .map_err(|error| error.to_string())?;
    let held_repository = crate::repository::verify_repository_at(repository_path, true)?;
    if held_repository.canonical_root()? != repository_root {
        return Err(
            "current repository changed while acquiring the verification import barrier".into(),
        );
    }
    ensure_pending_proposal(
        repository_path,
        &held_repository,
        &record.record.subject.proposal_id,
    )?;
    let (_, _, held_submission) = load_subject(repository_path, &held_repository, record)?;
    if held_submission.root.clone() != submission.root.clone() {
        return Err(
            "Verification source Submission changed while acquiring the import barrier".into(),
        );
    }

    let record_path = crate::submission::rooted_path("records/verifications/sha256", &record_root)?;
    let mut next_repository = held_repository.clone();
    crate::submission::add_object_ref(
        &mut next_repository.verifications,
        RepositoryObjectRefV1 {
            schema: record.record.schema.clone(),
            id: record.id.clone(),
            root: record_root.clone(),
            path: record_path.clone(),
        },
    )?;
    next_repository.verify()?;

    let recorded_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut prepared = crate::routine_evidence_transaction::prepare_routine_evidence_transaction(
        barrier,
        repository_path,
        &held_repository.repository_id,
        vela_repository::OperationKind::new(REPOSITORY_OPERATION_KIND)
            .map_err(|error| error.to_string())?,
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
                    digest: ContentDigest::parse(record.record.subject.submission_root.clone())
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
    )?;

    let (delta, preflight) = prepared.preflight_publication(
        repository_path,
        || Ok(PublishOptions::local()),
        "Verification import had no public Git delta",
    )?;
    prepared
        .mark_committed()
        .map_err(|error| error.to_string())?;
    prepared.install().map_err(|error| error.to_string())?;
    prepared.complete().map_err(|error| error.to_string())?;
    crate::repository::verify_repository_prepublication_at(repository_path)?;
    let publication = publish_exact_delta(
        repository_path,
        "verification import",
        std::slice::from_ref(&record.id),
        &delta,
        preflight,
    )?;
    if matches!(
        publication.state,
        PublicationState::Unchanged { .. } | PublicationState::CommittedLocal { .. }
    ) {
        crate::repository::verify_repository_at(repository_path, true).map_err(|error| {
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
        verification_record_id: record.id.clone(),
        verification_record_root: record_root,
        proposal_id: record.record.subject.proposal_id.clone(),
        claim_id: record.record.subject.claim_id.clone(),
        outcome: record.record.outcome.clone(),
        idempotent: false,
        accepted_event_delta: 0,
        publication,
    })
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;
    use vela_protocol::proposal::{ProposalProducerPackage, ProposalSubject};
    use vela_protocol::repository::REPOSITORY_SCHEMA_V4;
    use vela_protocol::signer_identity::{ActorClass, SignerIdentityV1};
    use vela_protocol::submission::{
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
        record: VerificationRecordEnvelopeV2,
        proposal_root: String,
    }

    fn write(repository_path: &Path, path: &str, bytes: &[u8]) {
        let absolute = repository_path.join(path);
        fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        fs::write(absolute, bytes).unwrap();
    }

    fn fixture() -> Fixture {
        let directory = TempDir::new().unwrap();
        let producer_key = SigningKey::from_bytes(&[51_u8; 32]);
        let producer_identity = SignerIdentityV1::new(
            "agent:producer-fixture",
            ActorClass::Agent,
            &producer_key,
            "2026-07-27T00:00:00Z",
        )
        .unwrap();
        let submission = SubmissionRecordV2::seal(
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
                    source_run: Some("run_fixture".into()),
                    emitted_at: "2026-07-27T00:00:00Z".into(),
                },
                execution_binding: None,
            },
            producer_identity,
            &producer_key,
        )
        .unwrap();
        let submission_root = submission.root.clone();
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
                kind: "submission".into(),
                id: submission.id.clone(),
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
            &submission.bytes.clone(),
        );
        write(
            directory.path(),
            &proposal_path,
            &proposal.canonical_bytes().unwrap(),
        );

        let verifier_key = SigningKey::from_bytes(&[52_u8; 32]);
        let verifier_identity = SignerIdentityV1::new(
            "service:verifier-fixture",
            ActorClass::Org,
            &verifier_key,
            "2026-07-27T00:00:00Z",
        )
        .unwrap();
        let record = VerificationRecordEnvelopeV2::seal(
            VerificationRecordDraft {
                subject: VerificationSubject {
                    claim_id,
                    artifact_ids: vec!["e".repeat(64)],
                    submission_id: submission.id.clone(),
                    submission_root: submission_root.clone(),
                    proposal_id: proposal.id(),
                    proposal_root: proposal.canonical_root().unwrap(),
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
            repository_id: "01234567-89ab-4def-8123-456789abcdef".into(),
            profile_root: root('1'),
            origin_id: vela_protocol::derive_handle("vro_", &root('2')).unwrap(),
            origin_root: root('2'),
            accepted_claims: Vec::new(),
            pending_claims: Vec::new(),
            proposals: vec![RepositoryObjectRefV1 {
                schema: proposal.schema.clone(),
                id: proposal.id(),
                root: proposal_root.clone(),
                path: proposal_path,
            }],
            proposal_withdrawals: Vec::new(),
            submissions: vec![RepositoryObjectRefV1 {
                schema: submission.submission.schema.clone(),
                id: submission.id,
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
            authority_model_root: root('4'),
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
        assert_eq!(proposal.subject.id, fixture.record.record.subject.claim_id);
        assert_eq!(
            submission.root.clone(),
            fixture.record.record.subject.submission_root
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
        let record_root = fixture.record.root.clone();
        let path =
            crate::submission::rooted_path("records/verifications/sha256", &record_root).unwrap();
        write(
            fixture._directory.path(),
            &path,
            &fixture.record.bytes.clone(),
        );
        fixture
            .repository
            .verifications
            .push(RepositoryObjectRefV1 {
                schema: fixture.record.record.schema.clone(),
                id: fixture.record.id.clone(),
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
            &fixture.record.record.subject,
            &fixture.record.record.method,
            &fixture.record.record.scope,
            &fixture.record.record.outcome,
            fixture.record.record.verifier(),
            &fixture.record.record.independence,
        ));

        let mut changed_scope = fixture.record.record.scope.clone();
        changed_scope.property.push_str(" changed");
        assert!(!matches_request(
            &fixture.record,
            &fixture.record.record.subject,
            &fixture.record.record.method,
            &changed_scope,
            &fixture.record.record.outcome,
            fixture.record.record.verifier(),
            &fixture.record.record.independence,
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
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(directory.path())
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "-q"]);
        fs::create_dir_all(directory.path().join("verification")).unwrap();
        let first = br#"{"schema":"fixture.method.v1","version":1}"#;
        let second = br#"{"schema":"fixture.method.v1","version":2}"#;
        fs::write(directory.path().join(path), first).unwrap();
        git(&["add", "verification/method.json"]);
        git(&[
            "-c",
            "user.name=Vela Test",
            "-c",
            "user.email=vela@example.invalid",
            "commit",
            "-qm",
            "method v1",
        ]);
        let (implementation, first_root, review_method) =
            method_manifest_binding(directory.path(), path).unwrap();
        assert_eq!(implementation, "verification/method.json");
        assert!(review_method.is_none());

        let untracked = Path::new("verification/untracked.json");
        fs::write(directory.path().join(untracked), b"untracked").unwrap();
        assert_eq!(
            method_manifest_binding(directory.path(), untracked).unwrap_err(),
            RETAINED_ERROR
        );
        git(&["add", "verification/untracked.json"]);
        assert_eq!(
            method_manifest_binding(directory.path(), untracked).unwrap_err(),
            DIRTY_ERROR
        );
        git(&["reset", "--", "verification/untracked.json"]);
        fs::remove_file(directory.path().join(untracked)).unwrap();

        fs::write(directory.path().join(path), second).unwrap();
        let error = method_manifest_binding(directory.path(), path).unwrap_err();
        assert_eq!(error, DIRTY_ERROR);
        git(&["add", "verification/method.json"]);
        fs::write(directory.path().join(path), first).unwrap();
        assert_eq!(
            method_manifest_binding(directory.path(), path).unwrap_err(),
            DIRTY_ERROR
        );
        git(&["reset", "--hard", "HEAD"]);

        #[cfg(unix)]
        {
            git(&["update-index", "--chmod=+x", "verification/method.json"]);
            assert_eq!(
                method_manifest_binding(directory.path(), path).unwrap_err(),
                DIRTY_ERROR
            );
            git(&["update-index", "--chmod=-x", "verification/method.json"]);

            let absolute = directory.path().join(path);
            let mut permissions = fs::metadata(&absolute).unwrap().permissions();
            let original_mode = permissions.mode();
            permissions.set_mode(original_mode | 0o111);
            fs::set_permissions(&absolute, permissions).unwrap();
            assert_eq!(
                method_manifest_binding(directory.path(), path).unwrap_err(),
                DIRTY_ERROR
            );
            let mut permissions = fs::metadata(&absolute).unwrap().permissions();
            permissions.set_mode(original_mode);
            fs::set_permissions(&absolute, permissions).unwrap();
        }

        fs::write(directory.path().join(path), second).unwrap();
        git(&["add", "verification/method.json"]);
        git(&[
            "-c",
            "user.name=Vela Test",
            "-c",
            "user.email=vela@example.invalid",
            "commit",
            "-qm",
            "method v2",
        ]);
        let (_, second_root, _) = method_manifest_binding(directory.path(), path).unwrap();
        assert_ne!(first_root, second_root);

        fs::write(directory.path().join(path), first).unwrap();
        git(&["add", "verification/method.json"]);
        assert_eq!(
            method_manifest_binding(directory.path(), path).unwrap_err(),
            DIRTY_ERROR
        );
        fs::write(directory.path().join(path), second).unwrap();
        git(&["add", "verification/method.json"]);

        git(&["rm", "--cached", "--", "verification/method.json"]);
        assert_eq!(
            method_manifest_binding(directory.path(), path).unwrap_err(),
            RETAINED_ERROR
        );
        git(&["add", "verification/method.json"]);

        #[cfg(unix)]
        {
            let sentinel = directory.path().join("filter-executed");
            let helper = directory.path().join("hostile-filter");
            fs::write(
                &helper,
                format!("#!/bin/sh\n: > '{}'\ncat\n", sentinel.display()),
            )
            .unwrap();
            let mut permissions = fs::metadata(&helper).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&helper, permissions).unwrap();
            fs::create_dir_all(directory.path().join(".git/info")).unwrap();
            for attributes in [
                directory.path().join(".gitattributes"),
                directory.path().join(".git/info/attributes"),
            ] {
                fs::write(attributes, b"verification/method.json filter=hostile\n").unwrap();
            }
            git(&["config", "filter.hostile.clean", helper.to_str().unwrap()]);
            git(&["config", "filter.hostile.smudge", helper.to_str().unwrap()]);

            method_manifest_binding(directory.path(), path).unwrap();
            assert!(!sentinel.exists(), "raw method reads executed a Git filter");
            fs::write(directory.path().join(path), b"dirty without filters").unwrap();
            assert_eq!(
                method_manifest_binding(directory.path(), path).unwrap_err(),
                DIRTY_ERROR
            );
            assert!(!sentinel.exists(), "refusal executed a Git filter");
            fs::write(directory.path().join(path), second).unwrap();
        }

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
    fn standard_review_method_is_typed_and_canonical() {
        let method = vela_protocol::review_method::ReviewMethodV1 {
            schema: REVIEW_METHOD_V1_SCHEMA.into(),
            profile: "statement-fidelity-gpt-5.6-sol-v1".into(),
            property: "statement_fidelity".into(),
            question: "Does the formal statement preserve the source question?".into(),
            reviewer: vela_protocol::review_method::ReviewPerformerV1 {
                kind: "ai_model".into(),
                display_name: "GPT-5.6 Sol".into(),
                identifier: "gpt-5.6-sol".into(),
                provider: Some("OpenAI".into()),
                version: None,
            },
            attested_by_actor_id: "agent:codex-review".into(),
            procedure: vec!["Compare the exact source and formal statement.".into()],
            required_output: vec!["Retain a witness for the first material mismatch.".into()],
            does_not_establish: vec!["Scientific acceptance or Standing.".into()],
        };
        let bytes = vela_protocol::canonical::to_canonical_bytes(&method).unwrap();
        assert_eq!(review_method_if_declared(&bytes).unwrap(), Some(method));

        let pretty = serde_json::to_vec_pretty(
            &serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        )
        .unwrap();
        assert!(
            review_method_if_declared(&pretty)
                .unwrap_err()
                .contains("canonical JSON")
        );
    }

    #[test]
    fn standard_review_method_refuses_profile_property_actor_and_nonclaim_drift() {
        let review_method = vela_protocol::review_method::ReviewMethodV1 {
            schema: REVIEW_METHOD_V1_SCHEMA.into(),
            profile: "statement-fidelity-gpt-5.6-sol-v1".into(),
            property: "statement_fidelity".into(),
            question: "Does the formal statement preserve the source question?".into(),
            reviewer: vela_protocol::review_method::ReviewPerformerV1 {
                kind: "ai_model".into(),
                display_name: "GPT-5.6 Sol".into(),
                identifier: "gpt-5.6-sol".into(),
                provider: Some("OpenAI".into()),
                version: None,
            },
            attested_by_actor_id: "agent:codex-review".into(),
            procedure: vec!["Compare the exact source and formal statement.".into()],
            required_output: vec!["Retain a scoped finding.".into()],
            does_not_establish: vec!["Scientific acceptance or Standing.".into()],
        };
        let method = VerificationMethod {
            profile: review_method.profile.clone(),
            implementation: "methods/review.json".into(),
            environment_root: root('e'),
        };
        let scope = VerificationScope {
            property: review_method.property.clone(),
            does_not_establish: review_method.does_not_establish.clone(),
        };
        assert!(
            ensure_review_method_binding(&review_method, &method, &scope, "agent:codex-review")
                .is_ok()
        );

        let mut wrong_method = method.clone();
        wrong_method.profile = "other".into();
        assert!(
            ensure_review_method_binding(
                &review_method,
                &wrong_method,
                &scope,
                "agent:codex-review"
            )
            .is_err()
        );
        let mut wrong_scope = scope.clone();
        wrong_scope.property = "other".into();
        assert!(
            ensure_review_method_binding(
                &review_method,
                &method,
                &wrong_scope,
                "agent:codex-review"
            )
            .is_err()
        );
        assert!(
            ensure_review_method_binding(&review_method, &method, &scope, "agent:other").is_err()
        );
        let mut missing_nonclaim = scope;
        missing_nonclaim.does_not_establish.clear();
        assert!(
            ensure_review_method_binding(
                &review_method,
                &method,
                &missing_nonclaim,
                "agent:codex-review"
            )
            .is_err()
        );
    }

    #[test]
    fn verification_actor_class_preserves_human_model_runner_and_org_provenance() {
        assert_eq!(
            verification_actor_class("human:william-blair").unwrap(),
            ActorClass::Human
        );
        assert_eq!(
            verification_actor_class("agent:codex-review").unwrap(),
            ActorClass::Agent
        );
        assert_eq!(
            verification_actor_class("verifier:lean").unwrap(),
            ActorClass::Agent
        );
        assert_eq!(
            verification_actor_class("org:example-lab").unwrap(),
            ActorClass::Org
        );
        assert!(verification_actor_class("reviewed-by-someone").is_err());
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
