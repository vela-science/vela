//! Producer-owned withdrawal of one still-pending Proposal.
//!
//! This routine evidence transaction appends one self-authenticated lifecycle
//! record and removes only the withdrawn Claim from the pending projection.
//! It never reads repository-authority custody, emits an Event, or changes
//! accepted Standing.

use std::fs;
use std::path::Path;

use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use serde_json::json;
use vela_protocol::canonical::sha256_root;
use vela_protocol::proposal::ProposalV1;
use vela_protocol::proposal_withdrawal::ProposalWithdrawalEnvelopeV2;
use vela_protocol::repository::{RepositoryObjectRefV1, RepositoryV4};
use vela_protocol::submission::SubmissionRecordV2;

use crate::authority_transaction::AuthorityObjectDraft;
use crate::config::git_publish::{
    PublicationOutcome, PublicationState, PublishOptions, publish_exact_delta,
};
use vela_repository::{ContentDigest, InputBinding, OperationId, OperationKind, WriteClass};

/* One name for the verb on every path. The success payload said
`proposal.withdraw` and the failure envelope said `proposal withdraw`, so a
caller switching on `command` saw two keys for one invocation, and neither
named a verb the CLI accepts: the path is `vela review withdraw`. */
const COMMAND: &str = "review.withdraw";
const REPOSITORY_OPERATION_KIND: &str = "proposal_withdrawal";

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProposalWithdrawalOutcome {
    schema: &'static str,
    ok: bool,
    command: &'static str,
    operation_id: String,
    withdrawal_id: String,
    withdrawal_root: String,
    proposal_id: String,
    submission_id: String,
    standing: &'static str,
    accepted_event_delta: u8,
    accepted_state_changed: bool,
    authority_used: bool,
    publication: PublicationOutcome,
}

fn read_exact(
    repository_path: &Path,
    reference: &RepositoryObjectRefV1,
) -> Result<Vec<u8>, String> {
    let bytes = fs::read(repository_path.join(&reference.path))
        .map_err(|error| format!("read object {}: {error}", reference.path))?;
    let root = sha256_root(&bytes);
    if root != reference.root {
        return Err(format!(
            "current object {} differs from its repository root",
            reference.path
        ));
    }
    Ok(bytes)
}

fn proposal_package(
    repository_path: &Path,
    repository: &RepositoryV4,
    proposal_id: &str,
) -> Result<(ProposalV1, String, SubmissionRecordV2), String> {
    let proposal_reference = repository
        .proposals
        .iter()
        .find(|reference| reference.id == proposal_id)
        .ok_or_else(|| format!("current repository has no exact Proposal {proposal_id}"))?;
    let proposal = ProposalV1::parse(&read_exact(repository_path, proposal_reference)?)?;
    if proposal.id() != proposal_reference.id
        || proposal.canonical_root()? != proposal_reference.root
    {
        return Err("stored Proposal differs from its repository reference".into());
    }
    let submission_reference = repository
        .submissions
        .iter()
        .find(|reference| {
            reference.id == proposal.producer_package.id
                && reference.root == proposal.producer_package.root
                && reference.path == proposal.producer_package.path
        })
        .ok_or_else(|| "Proposal does not bind one exact retained Submission".to_string())?;
    let submission =
        SubmissionRecordV2::parse(&read_exact(repository_path, submission_reference)?)?;
    if submission.id != submission_reference.id
        || submission.root.clone() != submission_reference.root
    {
        return Err("stored Submission differs from its repository reference".into());
    }
    Ok((proposal, proposal_reference.root.clone(), submission))
}

fn request_root(
    repository: &RepositoryV4,
    proposal_root: &str,
    submission_root: &str,
    actor: &str,
    reason: &str,
) -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&json!({
            "schema": "vela.proposal-withdrawal-request.v1",
            "repository_id": repository.repository_id,
            "origin_id": repository.origin_id,
            "repository_before": repository.canonical_root()?,
            "proposal_root": proposal_root,
            "submission_root": submission_root,
            "actor": actor,
            "reason": reason,
        }))?
    ))
}

fn existing_outcome(
    repository_path: &Path,
    repository: &RepositoryV4,
    proposal_id: &str,
    actor: &str,
    reason: &str,
) -> Result<Option<ProposalWithdrawalOutcome>, String> {
    let withdrawals =
        crate::repository::load_current_proposal_withdrawals(repository_path, repository)?;
    let Some(withdrawal) = withdrawals.get(proposal_id) else {
        return Ok(None);
    };
    if withdrawal.withdrawal.actor != actor || withdrawal.withdrawal.reason != reason {
        return Err(format!(
            "Proposal {proposal_id} is already withdrawn by {} with a different exact request",
            withdrawal.withdrawal.actor
        ));
    }
    let root = withdrawal.root.clone();
    let operation_id = OperationId::derive("proposal-withdraw", root.as_bytes());
    Ok(Some(ProposalWithdrawalOutcome {
        schema: "vela.proposal-withdrawal-result.v1",
        ok: true,
        command: COMMAND,
        operation_id: operation_id.as_str().into(),
        withdrawal_id: withdrawal.id.clone(),
        withdrawal_root: root,
        proposal_id: withdrawal.withdrawal.proposal_id.clone(),
        submission_id: withdrawal.withdrawal.submission_id.clone(),
        standing: "withdrawn",
        accepted_event_delta: 0,
        accepted_state_changed: false,
        authority_used: false,
        publication: PublicationOutcome {
            state: PublicationState::Uncommitted {
                candidate: None,
                reason: "Proposal is already withdrawn".into(),
            },
        },
    }))
}

pub(crate) fn withdraw(
    repository_path: &Path,
    proposal_id: &str,
    actor: &str,
    reason: &str,
) -> Result<ProposalWithdrawalOutcome, String> {
    let actor = actor.trim();
    let reason = reason.trim();
    if actor.is_empty() || actor != actor.trim() {
        return Err("proposal withdrawal actor must be exact non-empty text".into());
    }
    if reason.is_empty() {
        return Err("proposal withdrawal reason cannot be empty".into());
    }
    let repository = crate::repository::verify_repository_at(repository_path, true)?;
    crate::repository_ops::verify_repository_transaction_barrier_read_only(repository_path)?;
    if let Some(outcome) =
        existing_outcome(repository_path, &repository, proposal_id, actor, reason)?
    {
        return Ok(outcome);
    }
    if let Some(standing) =
        crate::repository::load_current_proposal_standings(repository_path, &repository)?
            .get(proposal_id)
    {
        return Err(format!(
            "Proposal {proposal_id} is {}, not pending_review",
            standing
        ));
    }
    let repository_root = repository.canonical_root()?;
    let (proposal, proposal_root, submission) =
        proposal_package(repository_path, &repository, proposal_id)?;
    if actor != proposal.actor
        || actor != submission.submission.provenance.producer
        || actor != submission.submission.identity.actor_id
    {
        return Err("proposal withdrawal actor must own the exact retained Submission".into());
    }

    let journal_dir = crate::repository_ops::repository_transaction_journal_dir(repository_path)?;
    let barrier = crate::repository_write_policy::acquire_routine_evidence_write_barrier(
        repository_path,
        &journal_dir,
    )
    .map_err(|error| error.to_string())?;
    let held = crate::repository::verify_repository_at(repository_path, true)?;
    if held.canonical_root()? != repository_root {
        return Err("current repository changed while acquiring the withdrawal barrier".into());
    }

    let created_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let key = vela_edge::agent_identity::existing_agent_signing_key(actor)?;
    let withdrawal = ProposalWithdrawalEnvelopeV2::seal(
        &proposal,
        &submission,
        actor.into(),
        reason.into(),
        created_at.clone(),
        &key,
    )?;
    let withdrawal_root = withdrawal.root.clone();
    let withdrawal_path =
        crate::submission::rooted_path("records/proposal-withdrawals/sha256", &withdrawal_root)?;

    let mut next = held.clone();
    if proposal.action == "claim.add" || proposal.action == "claim.revise" {
        let before = next.pending_claims.len();
        next.pending_claims.retain(|claim| {
            claim.claim_id != proposal.subject.id || claim.claim_root != proposal.subject.root
        });
        if next.pending_claims.len() + 1 != before {
            return Err("pending Proposal does not bind exactly one pending Claim".into());
        }
    }
    crate::submission::add_object_ref(
        &mut next.proposal_withdrawals,
        RepositoryObjectRefV1 {
            schema: withdrawal.withdrawal.schema.clone(),
            id: withdrawal.id.clone(),
            root: withdrawal_root.clone(),
            path: withdrawal_path.clone(),
        },
    )?;
    next.verify()?;
    let request_root = request_root(
        &held,
        &proposal_root,
        &submission.root.clone(),
        actor,
        reason,
    )?;
    let operation_id = OperationId::derive("proposal-withdraw", request_root.as_bytes());
    let read_set = vec![
        InputBinding {
            name: "current_repository_before".into(),
            digest: ContentDigest::parse(repository_root).map_err(|error| error.to_string())?,
        },
        InputBinding {
            name: "proposal".into(),
            digest: ContentDigest::parse(proposal_root).map_err(|error| error.to_string())?,
        },
        InputBinding {
            name: "submission".into(),
            digest: ContentDigest::parse(submission.root.clone())
                .map_err(|error| error.to_string())?,
        },
    ];
    let objects = vec![
        AuthorityObjectDraft {
            path: withdrawal_path,
            object_kind: "proposal_withdrawal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(withdrawal.bytes.clone()),
        },
        AuthorityObjectDraft {
            path: ".vela/repository.json".into(),
            object_kind: "repository_manifest".into(),
            class: WriteClass::CanonicalEvidence,
            postimage: Some(next.canonical_bytes()?),
        },
    ];
    let mut prepared = crate::routine_evidence_transaction::prepare_routine_evidence_transaction(
        barrier,
        repository_path,
        &held.repository_id,
        OperationKind::new(REPOSITORY_OPERATION_KIND).map_err(|error| error.to_string())?,
        operation_id.clone(),
        &request_root,
        created_at,
        read_set,
        objects,
    )?;
    let (delta, preflight) = prepared.preflight_publication(
        repository_path,
        || Ok(PublishOptions::local()),
        "Proposal Withdrawal transaction had no public Git delta",
    )?;
    prepared
        .mark_committed()
        .map_err(|error| error.to_string())?;
    prepared.install().map_err(|error| error.to_string())?;
    prepared.complete().map_err(|error| error.to_string())?;
    crate::repository::verify_repository_prepublication_at(repository_path)?;
    let publication = publish_exact_delta(
        repository_path,
        "proposal withdraw",
        std::slice::from_ref(&withdrawal.id),
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
                "Proposal Withdrawal was published but strict verification failed: {error}; do not retry"
            )
        })?;
        if let Err(error) = prepared.retire_completed_recovery_blobs() {
            crate::ui::warn_nonfatal(&format!(
                "Proposal Withdrawal {} was published and verified, but recovery blob cleanup failed: {error}",
                operation_id.as_str()
            ));
        }
    }
    Ok(ProposalWithdrawalOutcome {
        schema: "vela.proposal-withdrawal-result.v1",
        ok: true,
        command: COMMAND,
        operation_id: operation_id.as_str().into(),
        withdrawal_id: withdrawal.id,
        withdrawal_root,
        proposal_id: withdrawal.withdrawal.proposal_id,
        submission_id: withdrawal.withdrawal.submission_id,
        standing: "withdrawn",
        accepted_event_delta: 0,
        accepted_state_changed: false,
        authority_used: false,
        publication,
    })
}

pub(crate) fn cmd_withdraw(
    repository_path: &Path,
    proposal_id: &str,
    actor: &str,
    reason: &str,
    json_out: bool,
) {
    crate::ui::set_mode(COMMAND, json_out);
    crate::ui::require_initialized_repo(repository_path);
    let repository_path = crate::ui::canonicalize_repo(repository_path);
    let outcome = withdraw(&repository_path, proposal_id, actor, reason).unwrap_or_else(|error| {
        crate::ui::fail_if_recovery_required(&repository_path);
        crate::cli::fail_return(&error)
    });
    if json_out {
        crate::cli::print_json(&outcome);
    } else {
        println!("withdrawn · {}", outcome.proposal_id);
        println!("  record: {}", outcome.withdrawal_id);
        println!("  accepted state changed: no");
        println!("  authority used: no");
    }
}
