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
use sha2::Digest;
use vela_protocol::current_repository::{CurrentRepositoryV4, RepositoryObjectRefV1};
use vela_protocol::proposal_v1::ProposalV1;
use vela_protocol::proposal_withdrawal_v1::ProposalWithdrawalV1;
use vela_protocol::submission_v1::SubmissionV1;

use crate::authority_transaction::AuthorityObjectDraft;
use crate::config::git_publish::{
    PublicationOutcome, PublicationState, PublishOptions, exact_publication_preflight,
    publish_exact_delta,
};
use crate::repository_txn::{ContentDigest, InputBinding, OperationId, OperationKind, WriteClass};
use crate::repository_ops::publication_delta;

/* One name for the verb on every path. The success payload said
`proposal.withdraw` and the failure envelope said `proposal withdraw`, so a
caller switching on `command` saw two keys for one invocation, and neither
named a verb the CLI accepts: the path is `vela review withdraw`. */
const COMMAND: &str = "review.withdraw";

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

fn read_exact(frontier: &Path, reference: &RepositoryObjectRefV1) -> Result<Vec<u8>, String> {
    let bytes = fs::read(frontier.join(&reference.path))
        .map_err(|error| format!("read current object {}: {error}", reference.path))?;
    let root = format!("sha256:{}", hex::encode(sha2::Sha256::digest(&bytes)));
    if root != reference.root {
        return Err(format!(
            "current object {} differs from its repository root",
            reference.path
        ));
    }
    Ok(bytes)
}

fn proposal_package(
    frontier: &Path,
    repository: &CurrentRepositoryV4,
    proposal_id: &str,
) -> Result<(ProposalV1, String, SubmissionV1), String> {
    let proposal_reference = repository
        .proposals
        .iter()
        .find(|reference| reference.id == proposal_id)
        .ok_or_else(|| format!("current repository has no exact Proposal {proposal_id}"))?;
    let proposal = ProposalV1::parse(&read_exact(frontier, proposal_reference)?)?;
    if proposal.proposal_id != proposal_reference.id
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
    let submission = SubmissionV1::parse(&read_exact(frontier, submission_reference)?)?;
    if submission.submission_id != submission_reference.id
        || submission.canonical_root()? != submission_reference.root
    {
        return Err("stored Submission differs from its repository reference".into());
    }
    Ok((proposal, proposal_reference.root.clone(), submission))
}

fn request_root(
    repository: &CurrentRepositoryV4,
    proposal_root: &str,
    submission_root: &str,
    actor: &str,
    reason: &str,
) -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&json!({
            "schema": "vela.current-proposal-withdrawal-request.v1",
            "frontier_id": repository.frontier_id,
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
    frontier: &Path,
    repository: &CurrentRepositoryV4,
    proposal_id: &str,
    actor: &str,
    reason: &str,
) -> Result<Option<ProposalWithdrawalOutcome>, String> {
    let withdrawals =
        crate::current_repository::load_current_proposal_withdrawals(frontier, repository)?;
    let Some(withdrawal) = withdrawals.get(proposal_id) else {
        return Ok(None);
    };
    if withdrawal.actor != actor || withdrawal.reason != reason {
        return Err(format!(
            "Proposal {proposal_id} is already withdrawn by {} with a different exact request",
            withdrawal.actor
        ));
    }
    let root = withdrawal.canonical_root()?;
    let operation_id = OperationId::derive("proposal-withdraw", root.as_bytes());
    Ok(Some(ProposalWithdrawalOutcome {
        schema: "vela.proposal-withdrawal-result.v1",
        ok: true,
        command: COMMAND,
        operation_id: operation_id.as_str().into(),
        withdrawal_id: withdrawal.withdrawal_id.clone(),
        withdrawal_root: root,
        proposal_id: withdrawal.proposal_id.clone(),
        submission_id: withdrawal.submission_id.clone(),
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
    frontier: &Path,
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
    let repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    if let Some(outcome) = existing_outcome(frontier, &repository, proposal_id, actor, reason)? {
        return Ok(outcome);
    }
    if let Some(standing) =
        crate::current_repository::load_current_proposal_standings(frontier, &repository)?
            .get(proposal_id)
    {
        return Err(format!(
            "Proposal {proposal_id} is {}, not pending_review",
            standing
        ));
    }
    let repository_root = repository.canonical_root()?;
    let (proposal, proposal_root, submission) =
        proposal_package(frontier, &repository, proposal_id)?;
    if actor != proposal.actor
        || actor != submission.provenance.producer
        || actor != submission.authentication.identity_binding.actor_id
    {
        return Err("proposal withdrawal actor must own the exact retained Submission".into());
    }

    let journal_dir = crate::repository_ops::frontier_transaction_journal_dir(frontier)?;
    let barrier = crate::repository_txn::RepositoryTxn::acquire_routine_evidence_write_barrier(
        frontier,
        &journal_dir,
    )
    .map_err(|error| error.to_string())?;
    let held = crate::current_repository::verify_current_repository_at(frontier, true)?;
    if held.canonical_root()? != repository_root {
        return Err("current repository changed while acquiring the withdrawal barrier".into());
    }

    let created_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let key = vela_edge::agent_identity::existing_agent_signing_key(actor)?;
    let withdrawal = ProposalWithdrawalV1::build(
        &proposal,
        proposal_root.clone(),
        &submission,
        actor.into(),
        reason.into(),
        created_at.clone(),
        &key,
    )?;
    let withdrawal_root = withdrawal.canonical_root()?;
    let withdrawal_path = crate::current_submission::rooted_path(
        "records/proposal-withdrawals/sha256",
        &withdrawal_root,
    )?;

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
    crate::current_submission::add_object_ref(
        &mut next.proposal_withdrawals,
        RepositoryObjectRefV1 {
            schema: withdrawal.schema.clone(),
            id: withdrawal.withdrawal_id.clone(),
            root: withdrawal_root.clone(),
            path: withdrawal_path.clone(),
        },
    )?;
    next.verify()?;
    let derived = crate::current_submission::rebind_target_index(frontier, &next)?;
    let request_root = request_root(
        &held,
        &proposal_root,
        &submission.canonical_root()?,
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
            digest: ContentDigest::parse(submission.canonical_root()?)
                .map_err(|error| error.to_string())?,
        },
    ];
    let objects = vec![
        AuthorityObjectDraft {
            path: withdrawal_path,
            object_kind: "proposal_withdrawal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(withdrawal.canonical_bytes()?),
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
        frontier,
        &held.frontier_id,
        OperationKind::ProposalWithdrawal,
        operation_id.clone(),
        &request_root,
        created_at,
        read_set,
        objects,
        derived,
    )?;
    let precommit = (|| {
        let public = prepared
            .resolved_public_writes()
            .map_err(|error| error.to_string())?;
        let delta_root = prepared.canonical_delta_root().to_string();
        let delta = publication_delta(frontier, &delta_root, public)?
            .ok_or_else(|| "Proposal Withdrawal transaction had no public Git delta".to_string())?;
        let publish_options = PublishOptions::local();
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
    crate::current_repository::verify_current_repository_allow_derived_drift_at(frontier)?;
    let publication = publish_exact_delta(
        frontier,
        "proposal withdraw",
        std::slice::from_ref(&withdrawal.withdrawal_id),
        &delta,
        preflight,
    )
    .map_err(|error| error.to_string())?;
    if matches!(
        publication.state,
        PublicationState::Unchanged { .. } | PublicationState::CommittedLocal { .. }
    ) {
        crate::current_repository::verify_current_repository_at(frontier, true).map_err(|error| {
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
        withdrawal_id: withdrawal.withdrawal_id,
        withdrawal_root,
        proposal_id: withdrawal.proposal_id,
        submission_id: withdrawal.submission_id,
        standing: "withdrawn",
        accepted_event_delta: 0,
        accepted_state_changed: false,
        authority_used: false,
        publication,
    })
}

fn publication_error(outcome: PublicationOutcome) -> String {
    match outcome.state {
        PublicationState::Uncommitted { reason, .. } => reason,
        PublicationState::Unchanged { .. } | PublicationState::CommittedLocal { .. } => {
            "unexpected completed publication during preflight".into()
        }
    }
}

pub(crate) fn cmd_withdraw(
    frontier: &Path,
    proposal_id: &str,
    actor: &str,
    reason: &str,
    json_out: bool,
) {
    crate::ui::set_mode(COMMAND, json_out);
    crate::ui::require_initialized_repo(frontier);
    let frontier = crate::ui::canonicalize_repo(frontier);
    let outcome = withdraw(&frontier, proposal_id, actor, reason)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    if json_out {
        crate::cli::print_json(&outcome);
    } else {
        println!("withdrawn · {}", outcome.proposal_id);
        println!("  record: {}", outcome.withdrawal_id);
        println!("  accepted state changed: no");
        println!("  authority used: no");
    }
}
