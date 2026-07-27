//! Current-only Verification Record intake for Profile v2 repositories.
//!
//! Verification remains scoped authenticated evidence. This writer retains one
//! exact Verification Record, advances the current repository under an
//! object-only authority record, and changes no Claim standing.

use std::fs;
use std::path::Path;

use chrono::{SecondsFormat, Utc};
use serde_json::json;
use vela_authority::CedarEvaluationInput;
use vela_authority::runtime_authentication::{
    AuthenticationRequest, RuntimeSessionState, SignedVerificationRecordSession,
};
use vela_protocol::authority::PrincipalSnapshotV1;
use vela_protocol::current_repository::{CurrentRepositoryV2, RepositoryObjectRefV1};
use vela_protocol::principal_capability::PrincipalClass;
use vela_protocol::proposal_v1::ProposalV1;
use vela_protocol::repository_epoch::RepositoryBoundaryV1;
use vela_protocol::submission_v1::SubmissionV1;
use vela_protocol::verification_record::VerificationRecordV1;

use crate::authority_transaction::{
    AuthorityObjectDraft, AuthorityTransactionRequest, prepare_authority_transaction,
};
use crate::config::git_publish::{
    PublicationOutcome, PublicationState, PublishOptions, exact_publication_preflight,
    publication_disabled_reason, publication_is_busy, publish_exact_delta,
};
use crate::frontier_txn::{ContentDigest, InputBinding, WriteClass};
use crate::workflow::{
    VerificationImportOutcome, active_repository_signing_key, publication_delta,
};

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
    repository: &CurrentRepositoryV2,
    record: &VerificationRecordV1,
) -> Result<(ProposalV1, String, SubmissionV1), String> {
    let proposal_reference = repository
        .proposals
        .iter()
        .find(|reference| reference.id == record.subject.proposal_id)
        .ok_or_else(|| {
            format!(
                "Verification Record Proposal {} is not pending in the current repository",
                record.subject.proposal_id
            )
        })?;
    let proposal = read_exact_object(
        frontier,
        proposal_reference,
        ProposalV1::parse,
        ProposalV1::canonical_bytes,
    )?;
    let proposal_root = proposal.canonical_root()?;
    if proposal.proposal_id != proposal_reference.id
        || proposal_root != proposal_reference.root
        || proposal.subject.id != record.subject.claim_id
        || proposal.producer_package.id != record.subject.submission_id
        || proposal.producer_package.root != record.subject.submission_root
    {
        return Err(
            "Verification Record does not bind the current Proposal and producer package".into(),
        );
    }

    let submission_reference = repository
        .submissions
        .iter()
        .find(|reference| reference.id == record.subject.submission_id)
        .ok_or_else(|| {
            format!(
                "Verification Record Submission {} is absent from the current repository",
                record.subject.submission_id
            )
        })?;
    if submission_reference.root != record.subject.submission_root
        || submission_reference.path != proposal.producer_package.path
    {
        return Err("Verification Record Submission reference differs from the Proposal".into());
    }
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
    Ok((proposal, proposal_root, submission))
}

fn existing_outcome(
    frontier: &Path,
    repository: &CurrentRepositoryV2,
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
                reason: "exact Verification Record is already registered".into(),
            },
            recovery_command: None,
        },
    }))
}

pub(crate) fn import(
    frontier: &Path,
    record: &VerificationRecordV1,
    executor: &str,
    push: bool,
) -> Result<VerificationImportOutcome, String> {
    record.verify()?;
    let executor = executor.trim();
    if executor != record.verifier || executor != record.authentication.identity_binding.actor_id {
        return Err("verification import actor must match the Verification Record verifier".into());
    }

    let repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let (_proposal, proposal_root, _submission) = load_subject(frontier, &repository, record)?;
    let record_bytes = record.canonical_bytes()?;
    let record_root = record.canonical_root()?;
    let request_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&json!({
            "schema": "vela.current-verification-import-request.v1",
            "frontier_id": repository.frontier_id,
            "epoch_id": repository.epoch_id,
            "repository_before": repository_root,
            "verification_record_root": record_root,
        }))?
    );
    let operation_id =
        crate::frontier_txn::OperationId::derive("verification-import", request_root.as_bytes());
    if let Some(outcome) = existing_outcome(
        frontier,
        &repository,
        record,
        &record_root,
        operation_id.as_str(),
    )? {
        return Ok(outcome);
    }

    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)?;
    let barrier = crate::frontier_txn::FrontierTxn::acquire_repository_authority_write_barrier(
        frontier,
        &journal_dir,
    )
    .map_err(|error| error.to_string())?;
    let held_repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    if held_repository.canonical_root()? != repository_root {
        return Err(
            "current repository changed while acquiring the verification import barrier".into(),
        );
    }
    load_subject(frontier, &held_repository, record)?;

    let epoch_bytes = fs::read(frontier.join(".vela/epoch.json"))
        .map_err(|error| format!("read current repository epoch: {error}"))?;
    let epoch = RepositoryBoundaryV1::parse(&epoch_bytes)?;
    if epoch.canonical_bytes()? != epoch_bytes {
        return Err("current repository epoch is not canonical JSON".into());
    }
    let authority =
        crate::cli::load_current_repository_authority(frontier, &held_repository, &epoch)?;
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

    let record_path =
        crate::current_submission::rooted_path("records/verifications/sha256", &record_root)?;
    let mut next_repository = held_repository.clone();
    crate::current_submission::add_object_ref(
        &mut next_repository.verifications,
        RepositoryObjectRefV1 {
            schema: record.schema.clone(),
            id: record.verification_record_id.clone(),
            root: record_root.clone(),
            path: record_path.clone(),
        },
    )?;
    next_repository.verify()?;
    let derived_drafts =
        crate::current_submission::rebind_target_index(frontier, &next_repository)?;

    let recorded_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let authorization_input = CedarEvaluationInput {
        schema: authority.policy_material.schema.clone(),
        policies: authority.policy_material.policies.clone(),
        entities: authority.policy_material.entities.clone(),
        principal: format!(
            "Agent::{}",
            serde_json::to_string(executor).expect("actor ID serializes")
        ),
        principal_class: PrincipalClass::Agent,
        action: "verification_import".into(),
        resource: format!(
            "Frontier::{}",
            serde_json::to_string(&held_repository.frontier_id).expect("Frontier ID serializes")
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
    let mut prepared = prepare_authority_transaction(
        barrier,
        frontier,
        AuthorityTransactionRequest {
            history: authority.history,
            intent_digest: request_root,
            principal: PrincipalSnapshotV1 {
                principal_id: executor.into(),
                principal_class: PrincipalClass::Agent,
                display_name: None,
                affiliation: None,
                account_links: vec![executor.into()],
            },
            authentication_request: AuthenticationRequest {
                principal_id: executor.into(),
                principal_class: PrincipalClass::Agent,
                transaction_at: recorded_at.clone(),
            },
            runtime_session_state: RuntimeSessionState::default(),
            authorization_input,
            delegation: None,
            semantic_approvals: Vec::new(),
            event_drafts: Vec::new(),
            object_drafts: vec![
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
            next_authority_keyset: None,
            next_policy_bundle: None,
            next_policy_material: None,
            read_set: vec![
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
            ],
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
    let publish_options = if push {
        PublishOptions::pushing()
    } else {
        PublishOptions::new(false)
    };
    let publication_disabled = publication_disabled_reason(frontier, &publish_options);
    let delta = if publication_disabled.is_some() {
        None
    } else {
        publication_delta(frontier, &delta_root, public)?
    };
    let preflight = delta
        .as_ref()
        .map(|delta| exact_publication_preflight(frontier, delta, &publish_options))
        .transpose();
    let preflight = match preflight {
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
            crate::current_repository::verify_current_repository_at(frontier, true)?;
            return Ok(VerificationImportOutcome {
                schema: "vela.verification-import-result.v1",
                operation_id: operation_id.as_str().into(),
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
    crate::current_repository::verify_current_repository_at(frontier, true)?;
    let publication = match (delta.as_ref(), preflight) {
        (Some(delta), Some(preflight)) => publish_exact_delta(
            frontier,
            "verification import",
            std::slice::from_ref(&record.verification_record_id),
            delta,
            preflight,
            &publish_options,
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

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;
    use vela_protocol::current_repository::CURRENT_REPOSITORY_SCHEMA_V2;
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

    struct Fixture {
        _directory: TempDir,
        repository: CurrentRepositoryV2,
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
            crate::current_submission::rooted_path("records/submissions/sha256", &submission_root)
                .unwrap();
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
            None,
        )
        .unwrap();
        let proposal_root = proposal.canonical_root().unwrap();
        let proposal_path =
            crate::current_submission::rooted_path("records/proposals/sha256", &proposal_root)
                .unwrap();
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
                    artifact_ids: vec!["va_input".into()],
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
        let repository = CurrentRepositoryV2 {
            schema: CURRENT_REPOSITORY_SCHEMA_V2.into(),
            frontier_id: "vfr_0123456789abcdef".into(),
            profile_root: root('1'),
            epoch_id: "vre_0123456789abcdef".into(),
            epoch_root: root('2'),
            accepted_claims: Vec::new(),
            pending_claims: Vec::new(),
            proposals: vec![RepositoryObjectRefV1 {
                schema: proposal.schema.clone(),
                id: proposal.proposal_id,
                root: proposal_root.clone(),
                path: proposal_path,
            }],
            submissions: vec![RepositoryObjectRefV1 {
                schema: submission.schema,
                id: submission.submission_id,
                root: submission_root,
                path: submission_path,
            }],
            registrations: Vec::new(),
            verifications: Vec::new(),
            artifacts: vec![RepositoryObjectRefV1 {
                schema: "vela.artifact-record.v1".into(),
                id: "va_input".into(),
                root: root('e'),
                path: format!("records/artifacts/sha256/{}.json", "e".repeat(64)),
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
        assert!(error.contains("does not bind"), "{error}");
    }

    #[test]
    fn exact_registered_verification_is_idempotent_and_non_authoritative() {
        let mut fixture = fixture();
        let record_root = fixture.record.canonical_root().unwrap();
        let path =
            crate::current_submission::rooted_path("records/verifications/sha256", &record_root)
                .unwrap();
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
}
