//! Current-only producer intake for Profile v2 repositories.
//!
//! This path consumes an authenticated Submission and optional private
//! Attempt, creates current Claim/Proposal/Registration objects, advances the
//! repository manifest under one object-only authority record, and changes no
//! accepted scientific state.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use chrono::{Datelike, SecondsFormat, Utc};
use serde_json::json;
use vela_authority::CedarEvaluationInput;
use vela_authority::runtime_authentication::{
    AuthenticationRequest, RuntimeSessionState, SignedAgentSubmissionSession,
};
use vela_protocol::authority::PrincipalSnapshotV1;
use vela_protocol::claim_record::{
    ClaimAssertion, ClaimEvidenceRef, ClaimRecordV1, ClaimRelation, ClaimSource,
};
use vela_protocol::current_repository::{
    ClaimStandingRefV1, CurrentRepositoryV2, RepositoryObjectRefV1,
};
use vela_protocol::principal_capability::PrincipalClass;
use vela_protocol::proposal_v1::{ProposalProducerPackage, ProposalSubject, ProposalV1};
use vela_protocol::registration_record::{RegistrationRecordV1, RegistrationRoots};
use vela_protocol::repository_epoch::RepositoryBoundaryV1;
use vela_protocol::submission_v1::SubmissionV1;

use crate::authority_transaction::{
    AuthorityDerivedDraft, AuthorityObjectDraft, AuthorityTransactionRequest,
};
use crate::config::git_publish::{
    PublicationOutcome, PublicationState, PublishOptions, exact_publication_preflight,
    publication_disabled_reason, publication_is_busy, publish_exact_delta,
};
use crate::frontier_txn::{ContentDigest, InputBinding, WriteClass};
use crate::workflow::{
    PreparedSubmissionArtifacts, SubmitOutcome, active_repository_signing_key,
    prepare_submission_artifacts, publication_delta, submission_publication_inputs,
};

pub(crate) fn rooted_path(directory: &str, root: &str) -> Result<String, String> {
    let digest = root
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("{directory} object root is not sha256"))?;
    Ok(format!("{directory}/{digest}.json"))
}

fn proposal_set_root(proposals: &[RepositoryObjectRefV1]) -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&json!({
            "schema": "vela.proposal-set.v1",
            "proposals": proposals,
        }))?
    ))
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
    repository: &mut CurrentRepositoryV2,
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
    frontier: &Path,
    repository: &CurrentRepositoryV2,
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
    let bytes = fs::read(frontier.join(&reference.path))
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
    frontier: &Path,
    repository: &CurrentRepositoryV2,
    submission: &SubmissionV1,
) -> Result<ProposedChange, String> {
    let target = load_target_claim(frontier, repository, submission)?;
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
        None,
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
    frontier: &Path,
    repository: &CurrentRepositoryV2,
) -> Result<Vec<AuthorityDerivedDraft>, String> {
    let path = frontier.join("targets.json");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(&path).map_err(|error| format!("read current Target Index: {error}"))?;
    let mut index: vela_edge::target_index::TargetIndexV3 = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse Target Index v3: {error}"))?;
    index.validate()?;
    if index.canonical_bytes()? != bytes || index.repository.epoch_id != repository.epoch_id {
        return Err("current Target Index is not an exact canonical epoch member".into());
    }
    index.repository.repository_root = repository.canonical_root()?;
    index.index_root = index.computed_index_root()?;
    Ok(vec![AuthorityDerivedDraft {
        path: "targets.json".into(),
        postimage: Some(index.canonical_bytes()?),
    }])
}

fn existing_outcome(
    frontier: &Path,
    repository: &CurrentRepositoryV2,
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
    for reference in &repository.registrations {
        let bytes = fs::read(frontier.join(&reference.path))
            .map_err(|error| format!("read existing Registration Record: {error}"))?;
        let registration = RegistrationRecordV1::parse(&bytes)?;
        if registration.submission_id == submission.submission_id {
            matching.push((reference, registration));
        }
    }
    let [(reference, registration)] = matching.as_slice() else {
        return Err(format!(
            "registered Submission {} must have exactly one Registration Record; found {}",
            submission.submission_id,
            matching.len()
        ));
    };
    let proposal = repository
        .proposals
        .iter()
        .find(|proposal| proposal.id == registration.proposal_id)
        .ok_or_else(|| "existing Registration Record names a missing Proposal".to_string())?;
    let operation_id = crate::frontier_txn::OperationId::derive(
        "submit",
        registration.transaction_root.as_bytes(),
    );
    Ok(Some(SubmitOutcome {
        schema: "vela.submit-result.v1",
        operation_id: operation_id.as_str().into(),
        submission_id: submission.submission_id.clone(),
        submission_root: submission_root.to_string(),
        registration_record_id: registration.registration_record_id.clone(),
        registration_record_root: reference.root.clone(),
        proposal_id: proposal.id.clone(),
        claim_id: registration.claim_id.clone(),
        route: "pending_review",
        accepted_event_count_before: 0,
        accepted_event_count_after: 0,
        accepted_event_delta: 0,
        accepted_state_changed: false,
        publication: PublicationOutcome {
            state: PublicationState::Uncommitted {
                candidate: None,
                reason: "Submission is already registered in the current repository".into(),
            },
            recovery_command: None,
        },
    }))
}

pub(crate) fn submit(
    frontier: &Path,
    submission: &SubmissionV1,
    executor: &str,
    requested_attempt: Option<&str>,
    bundle_root: Option<&Path>,
    push: bool,
) -> Result<SubmitOutcome, String> {
    submission.verify()?;
    let executor = executor.trim();
    if executor != submission.provenance.producer
        || executor != submission.authentication.identity_binding.actor_id
    {
        return Err("submit actor must match the Submission producer identity".into());
    }
    if submission.provenance.source_attempt.as_deref() != requested_attempt {
        return Err(
            "Submission provenance.source_attempt must exactly match --attempt, including absence"
                .into(),
        );
    }

    let repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let submission_root = submission.canonical_root()?;
    if let Some(outcome) = existing_outcome(frontier, &repository, submission, &submission_root)? {
        let resolved_attempt =
            crate::current_work::resolve_submission_attempt(frontier, executor, requested_attempt)?;
        crate::current_work::close_submission_attempt(resolved_attempt)?;
        return Ok(outcome);
    }
    let resolved_attempt =
        crate::current_work::resolve_submission_attempt(frontier, executor, requested_attempt)?;

    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)?;
    let barrier = crate::frontier_txn::FrontierTxn::acquire_repository_authority_write_barrier(
        frontier,
        &journal_dir,
    )
    .map_err(|error| error.to_string())?;
    let held_repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    if held_repository.canonical_root()? != repository_root {
        return Err("current repository changed while acquiring the submit barrier".into());
    }
    if let Some(resolved) = &resolved_attempt {
        vela_edge::target_index::revalidate_current_target_task_binding(
            frontier,
            &resolved.attempt.target_task_binding,
        )?;
    }
    let epoch_bytes = fs::read(frontier.join(".vela/epoch.json"))
        .map_err(|error| format!("read current repository epoch: {error}"))?;
    let epoch = RepositoryBoundaryV1::parse(&epoch_bytes)?;
    if epoch.canonical_bytes()? != epoch_bytes {
        return Err("current repository epoch is not canonical JSON".into());
    }
    let authority =
        crate::cli::load_current_repository_authority(frontier, &held_repository, &epoch)?;

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
        "receipt_land"
    } else {
        return Err(
            "repository authority does not permit authenticated producer registration".into(),
        );
    };

    let fixed_time = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let PreparedSubmissionArtifacts {
        writes: artifact_writes,
        mut read_set,
    } = prepare_submission_artifacts(frontier, submission, bundle_root)?;
    read_set.push(InputBinding {
        name: "submission".into(),
        digest: ContentDigest::parse(submission_root.clone()).map_err(|error| error.to_string())?,
    });
    read_set.push(InputBinding {
        name: "current_repository_before".into(),
        digest: ContentDigest::parse(repository_root.clone()).map_err(|error| error.to_string())?,
    });
    if let Some(resolved) = &resolved_attempt {
        read_set.push(InputBinding {
            name: "current_attempt_binding".into(),
            digest: ContentDigest::parse(resolved.attempt.target_task_binding.binding_root.clone())
                .map_err(|error| error.to_string())?,
        });
    }

    let change = proposed_change(frontier, &held_repository, submission)?;
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
            "Register authenticated Submission {} for independent verification and authorized review.",
            submission.submission_id
        ),
        ProposalProducerPackage {
            kind: "submission_v1".into(),
            id: submission.submission_id.clone(),
            root: submission_root.clone(),
            path: submission_path.clone(),
        },
        submission.caveats.clone(),
        None,
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
    let request_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&json!({
            "schema": "vela.current-submit-request.v1",
            "frontier_id": held_repository.frontier_id,
            "epoch_id": held_repository.epoch_id,
            "repository_before": repository_root,
            "submission_root": submission_root,
            "attempt_binding_root": resolved_attempt
                .as_ref()
                .map(|resolved| &resolved.attempt.target_task_binding.binding_root),
        }))?
    );
    let operation_id = crate::frontier_txn::OperationId::derive("submit", request_root.as_bytes());
    let authority_event_root = authority.verification.final_event_log_root.clone();
    let registration = RegistrationRecordV1::build(
        held_repository.frontier_id.clone(),
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
        proposal.subject.id.clone(),
        proposal.proposal_id.clone(),
        "pending_review".into(),
        request_root.clone(),
        RegistrationRoots {
            event_log_before: authority_event_root.clone(),
            event_log_after: authority_event_root,
            proposal_after: proposal_set_root(&next_repository.proposals)?,
        },
        false,
    )?;
    let registration_root = registration.canonical_root()?;
    let registration_path = rooted_path("records/registrations/sha256", &registration_root)?;
    add_object_ref(
        &mut next_repository.registrations,
        RepositoryObjectRefV1 {
            schema: registration.schema.clone(),
            id: registration.registration_record_id.clone(),
            root: registration_root.clone(),
            path: registration_path.clone(),
        },
    )?;
    next_repository.verify()?;
    let derived_drafts = rebind_target_index(frontier, &next_repository)?;
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
            path: registration_path,
            object_kind: "registration_record".into(),
            class: WriteClass::PublicReview,
            postimage: Some(registration.canonical_bytes()?),
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

    let authorization_input = CedarEvaluationInput {
        schema: authority.policy_material.schema.clone(),
        policies: authority.policy_material.policies.clone(),
        entities: authority.policy_material.entities.clone(),
        principal: format!(
            "Agent::{}",
            serde_json::to_string(executor).expect("actor ID serializes")
        ),
        principal_class: PrincipalClass::Agent,
        action: registration_action.into(),
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
    let mut authentication = SignedAgentSubmissionSession::from_submission(submission)?;
    let mut prepared = crate::authority_transaction::prepare_authority_transaction(
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
                transaction_at: fixed_time.clone(),
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
            recorded_at: fixed_time,
        },
        &mut authentication,
        &mut repository_signer,
    )
    .map_err(|error| error.to_string())?;

    let public = prepared
        .resolved_public_writes()
        .map_err(|error| error.to_string())?;
    let delta_root = prepared.canonical_delta_root().to_string();
    let mut publish_options = if push {
        PublishOptions::pushing()
    } else {
        PublishOptions::new(false)
    };
    let publication_disabled = publication_disabled_reason(frontier, &publish_options);
    if publication_disabled.is_none() {
        publish_options = publish_options
            .with_preflight_inputs(submission_publication_inputs(frontier, submission)?);
    }
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
        Ok(preflight) => preflight,
        Err(outcome) if publication_is_busy(&outcome) => {
            return Err(
                "another Vela write/publication owns this repository; Submission was not registered"
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
            crate::current_work::close_submission_attempt(resolved_attempt)?;
            return Ok(SubmitOutcome {
                schema: "vela.submit-result.v1",
                operation_id: operation_id.as_str().into(),
                submission_id: submission.submission_id.clone(),
                submission_root,
                registration_record_id: registration.registration_record_id,
                registration_record_root: registration_root,
                proposal_id: proposal.proposal_id,
                claim_id: proposal.subject.id,
                route: "pending_review",
                accepted_event_count_before: 0,
                accepted_event_count_after: 0,
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
    crate::current_repository::verify_current_repository_at(frontier, true)?;
    crate::current_work::close_submission_attempt(resolved_attempt)?;
    let publication = match (delta.as_ref(), preflight) {
        (Some(delta), Some(preflight)) => publish_exact_delta(
            frontier,
            "submit",
            std::slice::from_ref(&proposal.proposal_id),
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
                    .unwrap_or_else(|| "Submission transaction had no public Git delta".into()),
            },
            recovery_command: None,
        },
    };
    Ok(SubmitOutcome {
        schema: "vela.submit-result.v1",
        operation_id: operation_id.as_str().into(),
        submission_id: submission.submission_id.clone(),
        submission_root,
        registration_record_id: registration.registration_record_id,
        registration_record_root: registration_root,
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
    use vela_protocol::current_repository::CURRENT_REPOSITORY_SCHEMA_V2;
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::submission_v1::{
        RequestedChange, RequestedChangeTarget, SubmissionArtifact, SubmissionClaim,
        SubmissionDraft, SubmissionProvenance,
    };

    use super::*;

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn repository() -> CurrentRepositoryV2 {
        CurrentRepositoryV2 {
            schema: CURRENT_REPOSITORY_SCHEMA_V2.into(),
            frontier_id: "vfr_0123456789abcdef".into(),
            profile_root: root('a'),
            epoch_id: "vre_0123456789abcdef".into(),
            epoch_root: root('b'),
            accepted_claims: Vec::new(),
            pending_claims: Vec::new(),
            proposals: Vec::new(),
            submissions: Vec::new(),
            registrations: Vec::new(),
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
        frontier: &Path,
        repository: &mut CurrentRepositoryV2,
        claim: &ClaimRecordV1,
    ) -> String {
        let claim_root = claim.canonical_root().unwrap();
        let path = rooted_path("records/claims/sha256", &claim_root).unwrap();
        let absolute = frontier.join(&path);
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
            None,
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
    fn add_submission_derives_a_pending_claim_without_accepted_state() {
        let frontier = TempDir::new().unwrap();
        let mut repository = repository();
        let change = proposed_change(
            frontier.path(),
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
        let frontier = TempDir::new().unwrap();
        let mut repository = repository();
        let original = accepted_claim();
        let original_root = install_accepted_claim(frontier.path(), &mut repository, &original);
        let change = proposed_change(
            frontier.path(),
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
        let frontier = TempDir::new().unwrap();
        let mut repository = repository();
        let original = accepted_claim();
        let original_root = install_accepted_claim(frontier.path(), &mut repository, &original);
        let change = proposed_change(
            frontier.path(),
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
        let frontier = TempDir::new().unwrap();
        let mut repository = repository();
        let original = accepted_claim();
        install_accepted_claim(frontier.path(), &mut repository, &original);
        let error = proposed_change(
            frontier.path(),
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
