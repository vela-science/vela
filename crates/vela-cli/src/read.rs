//! Object, standing, and authority-history readers.
//!
//! These projections use only the verified repository manifest, its
//! content-addressed records, and covered repository-authority history.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};
use vela_protocol::authority::AuthorityEventV1;
use vela_protocol::canonical::sha256_root;
use vela_protocol::claim_record::ClaimRecordV1;
use vela_protocol::proposal::ProposalV1;
use vela_protocol::proposal_withdrawal::ProposalWithdrawalEnvelopeV2;
use vela_protocol::repository::{ClaimStandingRefV1, RepositoryObjectRefV1, RepositoryV4};
use vela_protocol::repository_origin::RepositoryOriginV1;
use vela_protocol::repository_projection::{
    ProjectionActionV1, ProjectionArtifactV1, ProjectionAuthenticatedObjectV1,
    ProjectionAuthenticationV1, ProjectionAuthorityEventV1, ProjectionClaimV1,
    ProjectionDecisionV1, ProjectionEventRefV1, ProjectionFailedRouteV1, ProjectionHandoffV1,
    ProjectionProposalV1, ProjectionRepositoryV1, ProjectionTransitionV1,
    REPOSITORY_PROJECTION_AUTHORITY_EFFECT, REPOSITORY_PROJECTION_COMMAND,
    REPOSITORY_PROJECTION_ROOT_DEFINITION, REPOSITORY_PROJECTION_V1_SCHEMA, RepositoryProjectionV1,
};
use vela_protocol::review_method::{REVIEW_METHOD_V1_SCHEMA, ReviewMethodV1};
use vela_protocol::status::{
    REPOSITORY_HEAD_ROLE, ReplayState, StatusActions, StatusCounts, StatusDecisionInbox, StatusGit,
    StatusIntegrity, StatusReviewAction, StatusRoots, StatusWorkAction, StrictState,
};
use vela_protocol::submission::SubmissionRecordV3;
use vela_protocol::verification_record::VerificationRecordEnvelopeV2;

use crate::claim_standing::{self, ClaimStanding};
use crate::cli::{fail_return, print_json};
use crate::repository::ProposalDecision;

struct ReadContext {
    repository: RepositoryV4,
    repository_root: String,
    origin: RepositoryOriginV1,
    proposals: Vec<(RepositoryObjectRefV1, ProposalV1)>,
    decisions: BTreeMap<String, ProposalDecision>,
    withdrawals: BTreeMap<String, ProposalWithdrawalEnvelopeV2>,
    authority_events: Vec<AuthorityEventV1>,
    authority_record_root: String,
    authority_event_log_root: String,
}

fn canonical_root<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    vela_protocol::canonical::sha256_canonical(value).map(|digest| format!("sha256:{digest}"))
}

fn read_exact(repository_path: &Path, path: &str, expected_root: &str) -> Result<Vec<u8>, String> {
    let candidate = repository_path.join(path);
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|error| format!("inspect current object {path}: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("object {path} must be a regular file"));
    }
    let bytes = fs::read(&candidate).map_err(|error| format!("read object {path}: {error}"))?;
    if sha256_root(&bytes) != expected_root {
        return Err(format!(
            "current object {path} does not match {expected_root}"
        ));
    }
    Ok(bytes)
}

fn read_value(repository_path: &Path, reference: &RepositoryObjectRefV1) -> Result<Value, String> {
    let bytes = read_exact(repository_path, &reference.path, &reference.root)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse current object {}: {error}", reference.path))
}

fn load_context(repository_path: &Path) -> Result<ReadContext, String> {
    let repository = crate::repository::load_repository_at(repository_path, true)?;
    let repository_root = repository.canonical_root()?;
    let origin_bytes = fs::read(repository_path.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    let authority = crate::cli::load_repository_authority(repository_path, &repository, &origin)?;
    let decisions =
        crate::repository::load_current_proposal_decisions(repository_path, &repository)?;
    let withdrawals =
        crate::repository::load_current_proposal_withdrawals(repository_path, &repository)?;
    let proposals = repository
        .proposals
        .iter()
        .map(|reference| {
            let bytes = read_exact(repository_path, &reference.path, &reference.root)?;
            let proposal = ProposalV1::parse(&bytes)?;
            if proposal.id() != reference.id {
                return Err(format!(
                    "current Proposal {} does not match its repository reference",
                    reference.id
                ));
            }
            Ok((reference.clone(), proposal))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let authority_record_root = authority
        .verification
        .final_authority_record_root
        .clone()
        .ok_or_else(|| "current Repository has no authority-record head".to_string())?;
    Ok(ReadContext {
        repository,
        repository_root,
        origin,
        proposals,
        decisions,
        withdrawals,
        authority_events: authority.history.authority_events,
        authority_record_root,
        authority_event_log_root: authority.verification.final_event_log_root,
    })
}

/// The reference carries the manifest's own standing token. Reading that token
/// onto the standing axis is [`crate::claim_standing`]'s job, not this lookup's,
/// which is why the two lists are no longer labelled here.
fn claim_reference<'a>(context: &'a ReadContext, claim_id: &str) -> Option<&'a ClaimStandingRefV1> {
    context
        .repository
        .accepted_claims
        .iter()
        .chain(&context.repository.pending_claims)
        .find(|reference| reference.claim_id == claim_id)
}

/// The newest retained Proposal about this Claim.
fn latest_proposal<'a>(
    context: &'a ReadContext,
    claim_id: &str,
) -> Option<&'a (RepositoryObjectRefV1, ProposalV1)> {
    context
        .proposals
        .iter()
        .filter(|(_, proposal)| proposal.subject.id == claim_id)
        .max_by(|left, right| left.1.created_at.cmp(&right.1.created_at))
}

fn claim_proposal_status(context: &ReadContext, claim_id: &str) -> Option<String> {
    latest_proposal(context, claim_id).map(|(_, proposal)| proposal_status(context, &proposal.id()))
}

fn proposal_claim(
    repository_path: &Path,
    context: &ReadContext,
    claim_id: &str,
) -> Result<Option<ClaimRecordV1>, String> {
    let Some((_, proposal)) = latest_proposal(context, claim_id) else {
        return Ok(None);
    };
    let path = crate::submission::rooted_path("records/claims/sha256", &proposal.subject.root)?;
    let claim = ClaimRecordV1::parse(&read_exact(repository_path, &path, &proposal.subject.root)?)?;
    if claim.claim_id != proposal.subject.id {
        return Err(format!(
            "current Proposal {} resolves to the wrong Claim",
            proposal.id()
        ));
    }
    Ok(Some(claim))
}

/// A Proposal's own status, on the Proposal axis. This was `proposal_standing`
/// while its one caller handed the result straight back as a Claim's standing;
/// the name agreed with the collapse instead of exposing it.
fn proposal_status(context: &ReadContext, proposal_id: &str) -> String {
    context.decisions.get(proposal_id).map_or_else(
        || {
            if context.withdrawals.contains_key(proposal_id) {
                "withdrawn".into()
            } else {
                "pending_review".into()
            }
        },
        |decision| decision.standing.clone(),
    )
}

fn supersession_event<'a>(
    authority_events: &'a [AuthorityEventV1],
    claim_id: &str,
) -> Option<&'a AuthorityEventV1> {
    authority_events.iter().rev().find(|event| {
        event.content.kind.as_str() == "claim.superseded"
            && event.content.target.r#type == "claim"
            && event.content.target.id == claim_id
    })
}

fn superseded_claim(
    repository_path: &Path,
    context: &ReadContext,
    claim_id: &str,
) -> Result<Option<(ClaimRecordV1, String)>, String> {
    let Some(event) = supersession_event(&context.authority_events, claim_id) else {
        return Ok(None);
    };
    let root = event.content.before_hash.clone();
    if !root.starts_with("sha256:") || root == "sha256:null" {
        return Err(format!(
            "supersession event {} has no exact predecessor Claim root",
            event.id
        ));
    }
    let path = crate::submission::rooted_path("records/claims/sha256", &root)?;
    let claim = ClaimRecordV1::parse(&read_exact(repository_path, &path, &root)?)?;
    if claim.claim_id != claim_id {
        return Err(format!(
            "supersession event {} resolves to the wrong predecessor Claim",
            event.id
        ));
    }
    Ok(Some((claim, root)))
}

fn load_claim(
    repository_path: &Path,
    context: &ReadContext,
    claim_id: &str,
) -> Result<Option<(ClaimRecordV1, String, ClaimStanding)>, String> {
    let proposal_status = claim_proposal_status(context, claim_id);
    if let Some(reference) = claim_reference(context, claim_id) {
        let claim = ClaimRecordV1::parse(&read_exact(
            repository_path,
            &reference.path,
            &reference.claim_root,
        )?)?;
        let standing = ClaimStanding {
            standing: claim_standing::from_proposal_status(&reference.standing),
            proposal_status,
        };
        return Ok(Some((claim, reference.claim_root.clone(), standing)));
    }
    /* A retained accepted Proposal records the historical ruling on this
    Claim, but a later `claim.superseded` Event determines its current
    Standing. Resolve that Event before falling back to Proposal history so a
    retired predecessor cannot be projected as accepted merely because the
    Proposal that first admitted it is still retained. */
    if let Some((claim, root)) = superseded_claim(repository_path, context, claim_id)? {
        let standing = ClaimStanding {
            standing: claim_standing::SUPERSEDED,
            proposal_status,
        };
        return Ok(Some((claim, root, standing)));
    }
    if let Some(claim) = proposal_claim(repository_path, context, claim_id)? {
        let root = canonical_root(&claim)?;
        let standing = ClaimStanding {
            /* Reached only through a retained Proposal, so this Claim has one
            and the `unassessed` fallback is unreachable rather than a guess.
            The Proposal's action is read with its status because the manifest
            no longer answers here: the one way to reach this branch on a
            decided Proposal is an accepted `claim.withdraw`, which is exactly
            the case a verdict alone gets backwards. */
            standing: match (
                latest_proposal(context, claim_id),
                proposal_status.as_deref(),
            ) {
                (Some((_, proposal)), Some(status)) => {
                    claim_standing::from_proposal_outcome(&proposal.action, status)
                }
                _ => claim_standing::UNASSESSED,
            },
            proposal_status,
        };
        return Ok(Some((claim, root, standing)));
    }
    /* `Ok(None)` rather than `Err`: a miss and a broken Claim are different
    outcomes with different exit codes, and `show` has to distinguish them
    because it falls through to the other object kinds on a miss. */
    Ok(None)
}

fn supersession_view(context: &ReadContext, claim_id: &str) -> Result<Option<Value>, String> {
    let Some(event) = supersession_event(&context.authority_events, claim_id) else {
        return Ok(None);
    };
    let successor_claim_id = event
        .content
        .payload
        .get("claim_id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("supersession event {} has no successor Claim id", event.id))?;
    let proposal_id = event
        .content
        .payload
        .get("proposal_id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("supersession event {} has no Proposal id", event.id))?;
    Ok(Some(json!({
        "predecessor_claim_id": claim_id,
        "predecessor_claim_root": event.content.before_hash,
        "successor_claim_id": successor_claim_id,
        "successor_claim_root": event.content.after_hash,
        "proposal_id": proposal_id,
        "applied_authority_event_id": event.id,
        "applied_authority_event_root": event.root()?,
        "applied_semantic_event_id": event.semantic_event_id()?,
        "decision": context.decisions.get(proposal_id),
    })))
}

fn proposal_views(context: &ReadContext, claim_id: &str) -> Vec<Value> {
    context
        .proposals
        .iter()
        .filter(|(_, proposal)| proposal.subject.id == claim_id)
        .map(|(reference, proposal)| {
            json!({
                "proposal": proposal,
                "proposal_root": reference.root,
                "decision": context.decisions.get(&proposal.id()),
                "withdrawal": context.withdrawals.get(&proposal.id()).map(|value| &value.withdrawal),
            })
        })
        .collect()
}

/// The Verification Records about one Claim, as their verified payloads.
///
/// What is on disk is a DSSE envelope, so the payload this explanation is
/// about is base64 inside it and the subject it filters on is not reachable
/// from the stored JSON at all. Reading the file and pointing at
/// `/subject/claim_id` therefore matched nothing and silently explained every
/// Claim as having no Verification. Parse the envelope, which verifies the
/// signature against the key the payload declares, and project the payload.
fn verification_views(
    repository_path: &Path,
    context: &ReadContext,
    claim_id: &str,
) -> Result<Vec<Value>, String> {
    context
        .repository
        .verifications
        .iter()
        .map(|reference| {
            let bytes = read_exact(repository_path, &reference.path, &reference.root)?;
            VerificationRecordEnvelopeV2::parse(&bytes)
        })
        .filter_map(|result| match result {
            Ok(record) if record.record.subject.claim_id == claim_id => {
                Some(serde_json::to_value(&record.record).map_err(|error| {
                    format!("project current Verification Record {}: {error}", record.id)
                }))
            }
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

fn related_authority_events(context: &ReadContext, claim_id: &str) -> Result<Vec<Value>, String> {
    let proposal_ids = context
        .proposals
        .iter()
        .filter(|(_, proposal)| proposal.subject.id == claim_id)
        .map(|(_, proposal)| proposal.id())
        .collect::<Vec<_>>();
    context
        .authority_events
        .iter()
        .filter(|event| {
            event.content.target.id == claim_id
                || event
                    .content
                    .payload
                    .get("claim_id")
                    .and_then(Value::as_str)
                    == Some(claim_id)
                || event
                    .content
                    .payload
                    .get("proposal_id")
                    .and_then(Value::as_str)
                    .is_some_and(|proposal_id| {
                        proposal_ids.iter().any(|known| known == proposal_id)
                    })
        })
        .map(|event| {
            Ok(json!({
                "authority_event_id": event.id,
                "authority_event_root": event.root()?,
                "semantic_event_id": event.semantic_event_id()?,
                "event": event,
            }))
        })
        .collect()
}

fn object_projection(
    context: &ReadContext,
    object_id: &str,
    object_kind: &str,
    object_schema: &str,
    authority_effect: &str,
    content_root: String,
    object: Value,
) -> Value {
    json!({
        "ok": true,
        "command": "show",
        "schema": "vela.object-view.v1",
        "repository_id": context.repository.repository_id,
        "repository_root": context.repository_root,
        "object_id": object_id,
        "object_kind": object_kind,
        "object_schema": object_schema,
        "source_era": "current",
        "content_root": content_root,
        "authority_effect": authority_effect,
        "object": object,
    })
}

/// `show` gives an object one prose line for what it establishes, so a Claim's
/// line has to carry both axes: `unassessed` alone cannot tell a reader whether
/// a Decision rejected this Claim or nobody has looked at it yet.
fn claim_effect(standing: &ClaimStanding) -> String {
    let derived = format!(
        "scientific standing is {}, derived from current authority",
        standing.standing
    );
    match standing.proposal_status.as_deref() {
        Some(status) if standing.standing == claim_standing::UNASSESSED => format!(
            "{derived}; the Proposal about it is {status}, and no Decision put this Claim in accepted Standing"
        ),
        /* Naming the accepted Proposal without naming what it asked for reads
        as though the Claim were the thing accepted. What was accepted is its
        withdrawal, which is the only way this standing is reached. */
        Some(_) if standing.standing == claim_standing::RETRACTED => {
            format!("{derived}; a Decision accepted the Proposal to withdraw it")
        }
        Some(status) => format!("{derived}; the Proposal about it is {status}"),
        None => derived,
    }
}

pub(crate) fn show_payload(repository_path: &Path, object_id: &str) -> Result<Value, String> {
    let context = load_context(repository_path)?;
    if let Some(reference) = claim_reference(&context, object_id) {
        let standing = ClaimStanding {
            standing: claim_standing::from_proposal_status(&reference.standing),
            proposal_status: claim_proposal_status(&context, object_id),
        };
        return Ok(object_projection(
            &context,
            object_id,
            "claim",
            vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA,
            &claim_effect(&standing),
            reference.claim_root.clone(),
            serde_json::from_slice(&read_exact(
                repository_path,
                &reference.path,
                &reference.claim_root,
            )?)
            .map_err(|error| format!("parse current Claim: {error}"))?,
        ));
    }
    if object_id.starts_with("vcl_")
        && let Ok(Some((claim, root, standing))) = load_claim(repository_path, &context, object_id)
    {
        return Ok(object_projection(
            &context,
            object_id,
            "claim",
            vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA,
            &claim_effect(&standing),
            root,
            serde_json::to_value(claim).map_err(|error| error.to_string())?,
        ));
    }
    if let Some((reference, proposal)) = context
        .proposals
        .iter()
        .find(|(_, proposal)| proposal.id() == object_id)
    {
        return Ok(object_projection(
            &context,
            object_id,
            "proposal",
            vela_protocol::proposal::PROPOSAL_V1_SCHEMA,
            "requests a scientific-state change; a producer may withdraw it, but only an authorized Decision changes accepted Standing",
            reference.root.clone(),
            json!({
                "proposal": proposal,
                "decision": context.decisions.get(object_id),
                "withdrawal": context.withdrawals.get(object_id).map(|value| &value.withdrawal),
            }),
        ));
    }
    for (references, kind, effect) in [
        (
            &context.repository.submissions,
            "submission",
            "authenticated producer input; no accepted-state authority",
        ),
        (
            &context.repository.verifications,
            "verification_record",
            "verification observation; no accepted-state authority",
        ),
        (
            &context.repository.proposal_withdrawals,
            "proposal_withdrawal",
            "producer-owned pending lifecycle closure; no accepted-state authority",
        ),
        (
            &context.repository.artifacts,
            "artifact_record",
            "content provenance only; not verification or acceptance",
        ),
    ] {
        if let Some(reference) = references
            .iter()
            .find(|reference| reference.id == object_id)
        {
            return Ok(object_projection(
                &context,
                object_id,
                kind,
                &reference.schema,
                effect,
                reference.root.clone(),
                read_value(repository_path, reference)?,
            ));
        }
    }
    if let Some(event) = context.authority_events.iter().find(|event| {
        event.id == object_id
            || event
                .semantic_event_id()
                .is_ok_and(|semantic_id| semantic_id == object_id)
    }) {
        return Ok(object_projection(
            &context,
            object_id,
            "authority_event",
            vela_protocol::authority::AUTHORITY_EVENT_SCHEMA_V1,
            "covered repository-authority event; effect is determined by its exact kind and roots",
            event.root()?,
            json!({
                "authority_event": event,
                "semantic_event_id": event.semantic_event_id()?,
            }),
        ));
    }
    /* Diverges here rather than returning Err because this is the one place
    that knows the failure is a miss and not a broken object: every other exit
    from this function is a parse or integrity failure, and the caller receives
    both as the same `String`. */
    crate::cli::fail_kind(
        crate::ui::ErrorKind::NotFound,
        &format!("no exact current object '{object_id}' in this repository"),
    )
}

pub(crate) fn why_payload(repository_path: &Path, claim_id: &str) -> Result<Value, String> {
    let context = load_context(repository_path)?;
    let Some((claim, claim_root, standing)) = load_claim(repository_path, &context, claim_id)?
    else {
        crate::cli::fail_kind(
            crate::ui::ErrorKind::NotFound,
            &format!("no current or retained superseded Claim '{claim_id}' in this repository"),
        )
    };
    let proposals = proposal_views(&context, claim_id);
    let verification_records = verification_views(repository_path, &context, claim_id)?;
    let authority_events = related_authority_events(&context, claim_id)?;
    let supersession = supersession_view(&context, claim_id)?;
    Ok(json!({
        "ok": true,
        "command": "why",
        "schema": "vela.standing-explanation.v1",
        "repository_id": context.repository.repository_id,
        "repository_root": context.repository_root,
        "claim_id": claim_id,
        "claim_root": claim_root,
        /* Two axes, two fields, each named for the one it describes. `standing`
        answers "does this Claim stand?" in the Protocol vocabulary;
        `proposal_status` answers "what happened to the Proposal about
        it?" and is where `pending_review`, `rejected`, and `withdrawn` belong. */
        "standing": standing.standing,
        "proposal_status": standing.proposal_status,
        /* There is one authority chain and it starts at this repository's
        genesis, so a Claim stands on the live chain or it does not stand.
        `standing_basis` and the hop list beside it were the compaction answer:
        they distinguished a Claim decided here from one carried across a
        predecessor lineage, and that distinction went with the predecessor. */
        "chain": {
            "origin": {
                "origin_id": context.repository.origin_id,
                "origin_root": context.repository.origin_root,
                "generation": context.origin.generation,
                "initial_object_set_root": context.origin.initial_object_set_root,
            },
            "claim": claim,
            "proposals": proposals,
            "verification_records": verification_records,
            "authority_events": authority_events,
            "supersession": supersession,
        },
        "interpretation": {
            "submission_is_acceptance": false,
            "verification_is_acceptance": false,
            "standing_is_derived": true,
        },
    }))
}

pub(crate) fn log_payload(
    repository_path: &Path,
    object_id: Option<&str>,
    limit: usize,
    kind_filter: Option<&str>,
    as_of: Option<&str>,
) -> Result<Value, String> {
    let context = load_context(repository_path)?;
    let as_of = as_of.map(|value| {
        chrono::DateTime::parse_from_rfc3339(value).unwrap_or_else(|error| {
            crate::cli::fail_kind_return(
                crate::ui::ErrorKind::Usage,
                &format!("invalid --as-of timestamp {value:?}: {error}"),
            )
        })
    });
    let proposal_ids = object_id
        .map(|object_id| {
            context
                .proposals
                .iter()
                .filter(|(_, proposal)| {
                    proposal.id() == object_id || proposal.subject.id == object_id
                })
                .map(|(_, proposal)| proposal.id())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut events = context
        .authority_events
        .iter()
        .filter(|event| {
            kind_filter.is_none_or(|filter| event.content.kind.as_str().contains(filter))
        })
        .filter(|event| {
            object_id.is_none_or(|object_id| {
                event.id == object_id
                    || event.content.target.id == object_id
                    || event
                        .content
                        .payload
                        .get("claim_id")
                        .and_then(Value::as_str)
                        == Some(object_id)
                    || event
                        .content
                        .payload
                        .get("proposal_id")
                        .and_then(Value::as_str)
                        .is_some_and(|proposal_id| {
                            proposal_id == object_id
                                || proposal_ids.iter().any(|known| known == proposal_id)
                        })
            })
        })
        .filter(|event| {
            as_of.as_ref().is_none_or(|bound| {
                chrono::DateTime::parse_from_rfc3339(&event.content.timestamp)
                    .is_ok_and(|timestamp| timestamp <= *bound)
            })
        })
        .collect::<Vec<_>>();
    events.sort_by(|left, right| {
        right
            .content
            .timestamp
            .cmp(&left.content.timestamp)
            .then_with(|| right.id.cmp(&left.id))
    });
    events.truncate(limit);
    let events = events
        .into_iter()
        .map(|event| {
            Ok(json!({
                "id": event.semantic_event_id()?,
                "authority_event_id": event.id,
                "authority_event_root": event.root()?,
                "kind": event.content.kind,
                "actor": event.content.actor.id,
                "target": event.content.target.id,
                "target_type": event.content.target.r#type,
                "timestamp": event.content.timestamp,
                "reason": event.content.reason,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(json!({
        "ok": true,
        "command": "log",
        "schema": "vela.authority-log.v1",
        "repository_id": context.repository.repository_id,
        "repository_root": context.repository_root,
        "source_era": "current",
        "object_id": object_id,
        "as_of": as_of.map(|value| value.to_rfc3339()),
        "events": events,
    }))
}

fn projection_event_ref(event: &AuthorityEventV1) -> Result<ProjectionEventRefV1, String> {
    Ok(ProjectionEventRefV1 {
        authority_event_id: event.id.clone(),
        authority_event_root: event.root()?,
        semantic_event_id: event.semantic_event_id()?,
    })
}

fn authority_event_by_semantic_id<'a>(
    context: &'a ReadContext,
    semantic_id: &str,
) -> Result<&'a AuthorityEventV1, String> {
    context
        .authority_events
        .iter()
        .find(|event| event.semantic_event_id().is_ok_and(|id| id == semantic_id))
        .ok_or_else(|| format!("current authority history has no Event {semantic_id}"))
}

fn projection_decision(
    context: &ReadContext,
    decision: &ProposalDecision,
) -> Result<ProjectionDecisionV1, String> {
    let event = context
        .authority_events
        .iter()
        .find(|event| event.id == decision.event_id)
        .ok_or_else(|| {
            format!(
                "current Decision Event {} is absent from authority history",
                decision.event_id
            )
        })?;
    if event.root()? != decision.event_root {
        return Err(format!(
            "current Decision Event {} root drift",
            decision.event_id
        ));
    }
    let applied_event = decision
        .applied_event_id
        .as_deref()
        .map(|id| authority_event_by_semantic_id(context, id).and_then(projection_event_ref))
        .transpose()?;
    Ok(ProjectionDecisionV1 {
        verdict: decision.standing.clone(),
        decided_at: decision.decided_at.clone(),
        reason: decision.reason.clone(),
        actor_id: decision.actor.clone(),
        actor_class: decision.actor_class.clone(),
        session_ref: decision.session_ref.clone(),
        authority_principal_id: decision.authority_principal_id.clone(),
        repository_before: event
            .content
            .payload
            .get("repository_before")
            .and_then(Value::as_str)
            .map(str::to_owned),
        repository_after: event
            .content
            .payload
            .get("repository_after")
            .and_then(Value::as_str)
            .map(str::to_owned),
        decision_event: projection_event_ref(event)?,
        applied_event,
    })
}

fn correction_relation_kind(claim: &ClaimRecordV1, predecessor_id: &str) -> Option<String> {
    claim
        .relations
        .iter()
        .find(|relation| relation.target_claim_id == predecessor_id && relation.moves_standing())
        .map(|relation| relation.kind.clone())
}

fn projection_transition(
    repository_path: &Path,
    context: &ReadContext,
    claim: &ClaimRecordV1,
    claim_root: &str,
) -> Result<Option<ProjectionTransitionV1>, String> {
    /* A predecessor's current Standing comes from the admitted domain Event,
    not from the descriptive spelling on its successor. Preserve both facts. */
    if let Some(applied) = supersession_event(&context.authority_events, &claim.claim_id) {
        let successor_claim_id = applied
            .content
            .payload
            .get("claim_id")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("supersession Event {} has no successor", applied.id))?;
        let proposal_id = applied
            .content
            .payload
            .get("proposal_id")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("supersession Event {} has no Proposal", applied.id))?;
        let (successor, successor_root, _) =
            load_claim(repository_path, context, successor_claim_id)?
                .ok_or_else(|| format!("supersession Event {} successor is absent", applied.id))?;
        let decision = context
            .decisions
            .get(proposal_id)
            .ok_or_else(|| format!("supersession Event {} Decision is absent", applied.id))?;
        return Ok(Some(ProjectionTransitionV1 {
            authority_event_kind: applied.content.kind.as_str().to_owned(),
            relation_kind: correction_relation_kind(&successor, &claim.claim_id),
            predecessor_claim_id: Some(claim.claim_id.clone()),
            predecessor_claim_root: Some(claim_root.to_owned()),
            successor_claim_id: Some(successor_claim_id.to_owned()),
            successor_claim_root: Some(successor_root),
            proposal_id: proposal_id.to_owned(),
            decision_event: projection_decision(context, decision)?.decision_event,
            applied_event: Some(projection_event_ref(applied)?),
        }));
    }

    let Some((_, proposal)) = latest_proposal(context, &claim.claim_id) else {
        return Ok(None);
    };
    let Some(decision) = context.decisions.get(&proposal.id()) else {
        return Ok(None);
    };
    let Some(applied_id) = decision.applied_event_id.as_deref() else {
        return Ok(None);
    };
    let applied = authority_event_by_semantic_id(context, applied_id)?;
    let (authority_event_kind, relation_kind, predecessor_claim_id, predecessor_claim_root) =
        match applied.content.kind.as_str() {
            "claim.asserted" => ("claim.asserted", None, None, None),
            "claim.retracted" => (
                "claim.retracted",
                None,
                Some(claim.claim_id.clone()),
                Some(claim_root.to_owned()),
            ),
            "claim.superseded" => {
                let predecessor = claim
                    .relations
                    .iter()
                    .find(|relation| relation.moves_standing())
                    .ok_or_else(|| {
                        format!(
                            "accepted revision {} has no correction relation",
                            proposal.id()
                        )
                    })?;
                (
                    "claim.superseded",
                    Some(predecessor.kind.clone()),
                    Some(predecessor.target_claim_id.clone()),
                    Some(applied.content.before_hash.clone()),
                )
            }
            other => return Err(format!("unsupported scientific Event kind {other}")),
        };
    Ok(Some(ProjectionTransitionV1 {
        authority_event_kind: authority_event_kind.into(),
        relation_kind,
        predecessor_claim_id,
        predecessor_claim_root,
        successor_claim_id: (authority_event_kind != "claim.retracted")
            .then(|| claim.claim_id.clone()),
        successor_claim_root: (authority_event_kind != "claim.retracted")
            .then(|| claim_root.to_owned()),
        proposal_id: proposal.id(),
        decision_event: projection_decision(context, decision)?.decision_event,
        applied_event: Some(projection_event_ref(applied)?),
    }))
}

fn projection_review_method(
    repository_path: &Path,
    verification_id: &str,
    verification: &vela_protocol::verification_record::VerificationRecordV2,
) -> Result<Option<Value>, String> {
    let relative = Path::new(&verification.method.implementation);
    let bytes = match crate::bounded_file::read_bounded_repository_file(
        repository_path,
        relative,
        crate::bounded_file::PUBLIC_ARTIFACT_MAX_BYTES,
        "Review Method",
    ) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Ok(Some(json!({
                "state": "unavailable",
                "source_path": verification.method.implementation,
                "expected_root": verification.method.environment_root,
                "blocker_code": error.published_code(),
            })));
        }
    };
    let observed_root = sha256_root(&bytes);
    if observed_root != verification.method.environment_root {
        return Err(format!(
            "Verification Record {} Review Method root drift",
            verification_id
        ));
    }
    match ReviewMethodV1::parse_canonical(&bytes) {
        Ok(method) => {
            if method.profile != verification.method.profile
                || method.property != verification.scope.property
                || method.attested_by_actor_id != verification.identity.actor_id
                || !method
                    .does_not_establish
                    .iter()
                    .all(|value| verification.scope.does_not_establish.contains(value))
            {
                return Err(format!(
                    "Verification Record {} Review Method binding drift",
                    verification_id
                ));
            }
            Ok(Some(json!({
                "state": "verified",
                "schema": REVIEW_METHOD_V1_SCHEMA,
                "source_path": verification.method.implementation,
                "root": observed_root,
                "method": method,
            })))
        }
        Err(_) => Ok(Some(json!({
            "state": "opaque",
            "source_path": verification.method.implementation,
            "root": observed_root,
        }))),
    }
}

/// Build the one current-checkout semantic snapshot shared with read-only
/// consumers. It reuses verified Core parsers and performs no writes.
pub(crate) fn projection_payload(repository_path: &Path) -> Result<RepositoryProjectionV1, String> {
    let repository_path = crate::ui::canonicalize_repo(repository_path);
    let context = load_context(&repository_path)?;
    let profile_bytes = crate::bounded_file::read_bounded_repository_file(
        &repository_path,
        Path::new("vela.toml"),
        crate::bounded_file::PUBLIC_ARTIFACT_MAX_BYTES,
        "Repository Profile",
    )
    .map_err(|error| error.to_string())?;
    let profile_source = std::str::from_utf8(&profile_bytes)
        .map_err(|error| format!("repository Profile is not UTF-8: {error}"))?;
    let profile = vela_protocol::repository::RepositoryProfileV1::from_toml_str(profile_source)?;
    let git_commit =
        crate::repository::git_text(&repository_path, &["rev-parse", "HEAD^{commit}"])?;
    let git_tree = crate::repository::git_text(&repository_path, &["rev-parse", "HEAD^{tree}"])?;
    let inbox = crate::decision_inbox::project(&repository_path)?;

    let mut claim_ids = context
        .repository
        .accepted_claims
        .iter()
        .chain(&context.repository.pending_claims)
        .map(|reference| reference.claim_id.clone())
        .chain(
            context
                .proposals
                .iter()
                .map(|(_, proposal)| proposal.subject.id.clone()),
        )
        .collect::<std::collections::BTreeSet<_>>();
    let mut claims = Vec::with_capacity(claim_ids.len());
    let mut transitions_by_event = BTreeMap::new();
    for claim_id in std::mem::take(&mut claim_ids) {
        let (claim, claim_root, standing) = load_claim(&repository_path, &context, &claim_id)?
            .ok_or_else(|| format!("current Proposal resolves no Claim {claim_id}"))?;
        let active_reference = claim_reference(&context, &claim_id);
        let source_path = active_reference.map_or_else(
            || crate::submission::rooted_path("records/claims/sha256", &claim_root),
            |reference| Ok(reference.path.clone()),
        )?;
        if let Some(transition) =
            projection_transition(&repository_path, &context, &claim, &claim_root)?
        {
            transitions_by_event
                .entry(transition.decision_event.authority_event_root.clone())
                .or_insert(transition);
        }
        claims.push(ProjectionClaimV1 {
            claim_id: claim.claim_id.clone(),
            claim_root,
            source_path,
            active: active_reference.is_some(),
            standing: standing.standing.to_owned(),
            proposal_status: standing.proposal_status,
            assertion: claim.assertion.text.clone(),
            assertion_kind: claim.assertion.kind.clone(),
            record: serde_json::to_value(&claim).map_err(|error| error.to_string())?,
        });
    }

    let mut submissions = Vec::new();
    for reference in &context.repository.submissions {
        let parsed = SubmissionRecordV3::parse(&read_exact(
            &repository_path,
            &reference.path,
            &reference.root,
        )?)?;
        if parsed.id != reference.id || parsed.root != reference.root {
            return Err(format!(
                "Submission {} repository binding drift",
                reference.id
            ));
        }
        submissions.push(ProjectionAuthenticatedObjectV1 {
            object_id: parsed.id,
            object_root: parsed.root,
            source_path: reference.path.clone(),
            envelope: serde_json::to_value(&parsed.envelope).map_err(|error| error.to_string())?,
            payload: serde_json::to_value(&parsed.submission).map_err(|error| error.to_string())?,
            authentication: ProjectionAuthenticationV1 {
                signature_verified: true,
                actor_id: parsed.submission.identity.actor_id.clone(),
            },
            review_method: None,
        });
    }

    let mut parsed_verifications = Vec::new();
    let mut verifications = Vec::new();
    for reference in &context.repository.verifications {
        let parsed = VerificationRecordEnvelopeV2::parse(&read_exact(
            &repository_path,
            &reference.path,
            &reference.root,
        )?)?;
        if parsed.id != reference.id || parsed.root != reference.root {
            return Err(format!(
                "Verification Record {} repository binding drift",
                reference.id
            ));
        }
        let review_method = projection_review_method(&repository_path, &parsed.id, &parsed.record)?;
        verifications.push(ProjectionAuthenticatedObjectV1 {
            object_id: parsed.id.clone(),
            object_root: parsed.root.clone(),
            source_path: reference.path.clone(),
            envelope: serde_json::to_value(&parsed.envelope).map_err(|error| error.to_string())?,
            payload: serde_json::to_value(&parsed.record).map_err(|error| error.to_string())?,
            authentication: ProjectionAuthenticationV1 {
                signature_verified: true,
                actor_id: parsed.record.identity.actor_id.clone(),
            },
            review_method,
        });
        parsed_verifications.push(parsed);
    }

    let mut proposals = Vec::new();
    for (reference, proposal) in &context.proposals {
        let proposal_id = proposal.id();
        let status = proposal_status(&context, &proposal_id);
        let (_, _, subject_standing) =
            load_claim(&repository_path, &context, &proposal.subject.id)?
                .ok_or_else(|| format!("Proposal {proposal_id} resolves no subject Claim"))?;
        let verification_record_ids = parsed_verifications
            .iter()
            .filter(|record| record.record.subject.proposal_id == proposal_id)
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();
        let decision = context
            .decisions
            .get(&proposal_id)
            .map(|decision| projection_decision(&context, decision))
            .transpose()?;
        let entry = inbox
            .entries
            .iter()
            .find(|entry| entry.proposal_id == proposal_id);
        let consequence = entry.map_or_else(
            || json!({
                "state": "historical_base_required",
                "detail": "The current checkout retains the terminal Decision, but its exact pre-Decision Inbox belongs to the parent checkout.",
            }),
            |entry| json!({
                "state": "current_decision_inbox",
                "transition": entry.standing_delta.transition,
                "affected_claim_ids": entry.standing_delta.scope.affected_claim_ids,
                "before_repository_root": entry.standing_delta.before.repository_root,
                "if_accept_repository_root": entry.standing_delta.if_accept.repository_root,
                "if_reject_repository_root": entry.standing_delta.if_reject.repository_root,
                "blockers": entry.readiness.blockers,
                "next_obligation": entry.next_obligation,
            }),
        );
        proposals.push(ProjectionProposalV1 {
            proposal_id: proposal_id.clone(),
            proposal_root: reference.root.clone(),
            source_path: reference.path.clone(),
            status,
            subject_standing: subject_standing.standing.to_owned(),
            submission_id: proposal.producer_package.id.clone(),
            submission_root: proposal.producer_package.root.clone(),
            verification_record_ids,
            record: serde_json::to_value(proposal).map_err(|error| error.to_string())?,
            decision,
            withdrawal: context
                .withdrawals
                .get(&proposal_id)
                .map(|value| serde_json::to_value(&value.withdrawal))
                .transpose()
                .map_err(|error| error.to_string())?,
            decision_inbox_entry: entry
                .map(serde_json::to_value)
                .transpose()
                .map_err(|error| error.to_string())?,
            consequence,
        });
    }

    let mut artifacts = Vec::new();
    for reference in &context.repository.artifacts {
        let bytes = read_exact(&repository_path, &reference.path, &reference.root)?;
        artifacts.push(ProjectionArtifactV1 {
            artifact_id: reference.id.clone(),
            artifact_root: reference.root.clone(),
            source_path: reference.path.clone(),
            byte_length: bytes.len() as u64,
        });
    }

    let authority_events = context
        .authority_events
        .iter()
        .map(|event| {
            Ok(ProjectionAuthorityEventV1 {
                event: projection_event_ref(event)?,
                kind: event.content.kind.as_str().to_owned(),
                actor_id: event.content.actor.id.clone(),
                target_id: event.content.target.id.clone(),
                target_type: event.content.target.r#type.clone(),
                timestamp: event.content.timestamp.clone(),
                record: serde_json::to_value(event).map_err(|error| error.to_string())?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let pending_review = proposals
        .iter()
        .filter(|proposal| proposal.status == "pending_review")
        .count();
    let protocol_ready_count = inbox
        .entries
        .iter()
        .filter(|entry| entry.readiness.blockers.is_empty())
        .count();
    let review_action = (!inbox.entries.is_empty()).then(|| StatusReviewAction {
        pending_count: inbox.entries.len() as u64,
        command: "vela review inbox . --json".into(),
    });
    let actions = StatusActions {
        review: review_action,
        work: StatusWorkAction::DirectSubmission {
            command: "vela submit --repo . --help".into(),
            note: "Submit bounded evidence directly.".into(),
        },
    };

    let mut accepted_claim_ids = Vec::new();
    let mut active_pending_claim_ids = Vec::new();
    let mut inactive_unassessed_claim_ids = Vec::new();
    let mut retired_claim_ids = Vec::new();
    let mut correction_successor_ids = Vec::new();
    for claim in &claims {
        match claim.standing.as_str() {
            "accepted" => accepted_claim_ids.push(claim.claim_id.clone()),
            "unassessed" if claim.active => active_pending_claim_ids.push(claim.claim_id.clone()),
            "unassessed" => inactive_unassessed_claim_ids.push(claim.claim_id.clone()),
            "superseded" | "retracted" => retired_claim_ids.push(claim.claim_id.clone()),
            other => return Err(format!("unsupported projected Claim Standing {other}")),
        }
    }
    let transitions = transitions_by_event.into_values().collect::<Vec<_>>();
    correction_successor_ids.extend(transitions.iter().filter_map(|transition| {
        transition
            .relation_kind
            .as_ref()
            .and(transition.successor_claim_id.as_ref())
            .cloned()
    }));
    correction_successor_ids.sort();
    correction_successor_ids.dedup();
    let failed_routes = inbox
        .entries
        .iter()
        .flat_map(|entry| {
            entry
                .readiness
                .blockers
                .iter()
                .map(|blocker| ProjectionFailedRouteV1 {
                    code: blocker.code.clone(),
                    subject: blocker.subject.clone(),
                    detail: blocker.detail.clone(),
                })
        })
        .collect::<Vec<_>>();
    let exact_next_actions = actions
        .review
        .iter()
        .map(|action| ProjectionActionV1 {
            kind: "review".into(),
            command: action.command.clone(),
            note: None,
        })
        .chain(std::iter::once(ProjectionActionV1 {
            kind: "submission".into(),
            command: actions.work.command().to_owned(),
            note: Some("Submit bounded evidence directly.".into()),
        }))
        .collect::<Vec<_>>();
    let correction_impacts = correction_successor_ids
        .iter()
        .map(|claim_id| {
            crate::correction_impact::correction_impact_payload(&repository_path, claim_id)
                .map(|(payload, _)| payload)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut projection = RepositoryProjectionV1 {
        schema: REPOSITORY_PROJECTION_V1_SCHEMA.into(),
        ok: true,
        command: REPOSITORY_PROJECTION_COMMAND.into(),
        authority_effect: REPOSITORY_PROJECTION_AUTHORITY_EFFECT.into(),
        projection_root: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .into(),
        projection_root_definition: REPOSITORY_PROJECTION_ROOT_DEFINITION.into(),
        reader_version: env!("CARGO_PKG_VERSION").into(),
        repository: ProjectionRepositoryV1 {
            repository_id: context.repository.repository_id.clone(),
            name: profile.name,
            profile_root: context.repository.profile_root.clone(),
            origin_id: context.repository.origin_id.clone(),
            origin_root: context.repository.origin_root.clone(),
            origin_generation: context.origin.generation,
            initial_object_set_root: context.origin.initial_object_set_root.clone(),
            repository_index_path: ".vela/repository.json".into(),
            repository_root: context.repository_root.clone(),
            authority_keyset_root: context.repository.authority_keyset_root.clone(),
            authority_policy_root: context.repository.authority_model_root.clone(),
            authority_record_root: context.authority_record_root.clone(),
            authority_event_log_root: context.authority_event_log_root.clone(),
        },
        git: StatusGit {
            role: REPOSITORY_HEAD_ROLE.into(),
            commit: Some(git_commit),
            tree: Some(git_tree),
        },
        integrity: StatusIntegrity {
            replay: ReplayState::Verified,
            strict: StrictState::Pass,
            blocker_count: 0,
            blockers_by_code: BTreeMap::new(),
        },
        roots: StatusRoots {
            origin: Some(context.repository.origin_root.clone()),
            repository: Some(context.repository_root.clone()),
            authority_keyset: Some(context.repository.authority_keyset_root.clone()),
            authority_policy: Some(context.repository.authority_model_root.clone()),
        },
        counts: StatusCounts {
            claims: (context.repository.accepted_claims.len()
                + context.repository.pending_claims.len()) as u64,
            accepted_claims: accepted_claim_ids.len() as u64,
            pending_claims: context.repository.pending_claims.len() as u64,
            pending_review: pending_review as u64,
            accepted_review: proposals
                .iter()
                .filter(|value| value.status == "accepted")
                .count() as u64,
            rejected_review: proposals
                .iter()
                .filter(|value| value.status == "rejected")
                .count() as u64,
            withdrawn_review: proposals
                .iter()
                .filter(|value| value.status == "withdrawn")
                .count() as u64,
            submissions: submissions.len() as u64,
            verifications: verifications.len() as u64,
            artifacts: artifacts.len() as u64,
        },
        decision_inbox_summary: StatusDecisionInbox {
            pending_count: inbox.entries.len() as u64,
            protocol_ready_count: protocol_ready_count as u64,
            protocol_blocked_count: (inbox.entries.len() - protocol_ready_count) as u64,
            projection_root: Some(inbox.projection_root.clone()),
            first_entry_root: inbox.entries.first().map(|entry| entry.entry_root.clone()),
        },
        actions,
        claims,
        proposals,
        submissions,
        verifications,
        artifacts,
        authority_events,
        transitions,
        correction_impacts,
        decision_inbox: serde_json::to_value(&inbox).map_err(|error| error.to_string())?,
        handoff: ProjectionHandoffV1 {
            accepted_claim_ids,
            active_pending_claim_ids,
            inactive_unassessed_claim_ids,
            retired_claim_ids,
            pending_proposal_ids: inbox
                .entries
                .iter()
                .map(|entry| entry.proposal_id.clone())
                .collect(),
            correction_successor_ids,
            exact_next_actions,
            failed_routes,
            nonclaims: vec![
                "This read projection does not execute a Method or reproduce scientific evidence."
                    .into(),
                "A Submission, Verification, Git commit, or projection does not change Standing."
                    .into(),
                "Only an authorized Repository Decision changes accepted Standing.".into(),
            ],
        },
    };
    let mut commitment = serde_json::to_value(&projection).map_err(|error| error.to_string())?;
    commitment
        .as_object_mut()
        .ok_or_else(|| "repository projection is not an object".to_string())?
        .remove("projection_root");
    projection.projection_root = canonical_root(&commitment)?;
    Ok(projection)
}

pub(crate) fn cmd_projection(repository_path: &Path, json_out: bool) {
    crate::ui::set_mode("projection", json_out);
    crate::ui::require_initialized_repo(repository_path);
    let payload =
        projection_payload(repository_path).unwrap_or_else(|error| crate::cli::fail_return(&error));
    if json_out {
        print_json(&payload);
    } else {
        println!("{}", payload.repository.name);
        println!("  state   {}", payload.repository.repository_root);
        println!("  claims  {}", payload.claims.len());
        println!("  inbox   {}", payload.decision_inbox_summary.pending_count);
        println!("  effect  none");
    }
}

pub(crate) fn cmd_show(repository_path: &Path, object_id: &str, json_out: bool) {
    crate::ui::set_mode("show", json_out);
    crate::ui::require_initialized_repo(repository_path);
    let projection =
        show_payload(repository_path, object_id).unwrap_or_else(|error| fail_return(&error));
    if json_out {
        print_json(&projection);
    } else {
        render_show(&projection);
    }
}

/// Render `show` for a person: what this object is, what it says, and what it
/// establishes. The full record stays one `--json` away.
fn render_show(projection: &Value) {
    fn text(value: &Value) -> &str {
        value.as_str().unwrap_or("not recorded")
    }
    let kind = text(&projection["object_kind"]);
    println!("show · {} · {kind}", text(&projection["object_id"]));

    let object = &projection["object"];
    /* Each object kind has one line that is the thing itself. A Claim is its
    assertion, a Verification Record its scope, an Event its reason. */
    let subject = object["assertion"]["text"]
        .as_str()
        .or_else(|| object["scope"]["property"].as_str())
        .or_else(|| object["content"]["reason"].as_str())
        .or_else(|| object["reason"].as_str());
    if let Some(subject) = subject {
        println!("  says      {subject}");
    }

    println!("  schema    {}", text(&projection["object_schema"]));
    println!("  root      {}", text(&projection["content_root"]));
    println!("  era       {}", text(&projection["source_era"]));
    println!("  effect    {}", text(&projection["authority_effect"]));
}

pub(crate) fn cmd_why(repository_path: &Path, claim_id: &str, json_out: bool) {
    crate::ui::set_mode("why", json_out);
    crate::ui::require_initialized_repo(repository_path);
    if !claim_id.starts_with("vcl_") {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Usage,
            &format!("why explains a Claim, and {claim_id} is not a Claim id"),
            Some("use `vela why vcl_... --json`; `vela show` reads every other object kind"),
        );
    }
    let projection =
        why_payload(repository_path, claim_id).unwrap_or_else(|error| fail_return(&error));
    if json_out {
        print_json(&projection);
    } else {
        render_why(&projection);
    }
}

/// Render `why` for a person.
///
/// Both branches used to print the same pretty JSON, so the verb whose whole
/// purpose is answering "why does this stand" answered with 260 lines. The
/// reader wants four things: what stands, who decided it and what they wrote,
/// what was checked, and what none of that establishes.
fn render_why(projection: &Value) {
    let text = |value: &Value| value.as_str().unwrap_or("not recorded").to_string();
    let claim_id = text(&projection["claim_id"]);
    let standing = text(&projection["standing"]);
    /* The header carries both axes because the standing alone no longer says
    what became of the Proposal, and that is usually the reader's next question
    on anything that does not stand. */
    let proposal_status = projection["proposal_status"]
        .as_str()
        .map(|status| format!(" · proposal {status}"))
        .unwrap_or_default();
    println!("why · {claim_id} · {standing}{proposal_status}");

    let chain = &projection["chain"];
    /* `assertion` is an object — the text and its kind — not a string. */
    if let Some(assertion) = chain["claim"]["assertion"]["text"].as_str() {
        println!("  claim     {assertion}");
    }

    /* The Decision is the only act that changed Standing, so it leads. A
    Proposal may carry none, which is itself the answer to "why". */
    let decided = chain["proposals"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|entry| entry["decision"].as_object());
    match decided {
        Some(decision) => {
            let actor = decision
                .get("actor")
                .and_then(Value::as_str)
                .unwrap_or("actor not recorded");
            let at = decision
                .get("decided_at")
                .and_then(Value::as_str)
                .unwrap_or("time not recorded");
            /* The Decision's own verdict, not the Claim's standing: this line
            reports what the ruling said, and a rejection is a ruling. */
            let verdict = decision
                .get("standing")
                .and_then(Value::as_str)
                .unwrap_or("verdict not recorded");
            println!("  decided   {verdict} by {actor} at {at}");
            if let Some(reason) = decision.get("reason").and_then(Value::as_str) {
                println!("  because   {reason}");
            }
        }
        None => println!("  decided   no Decision is retained for this Claim"),
    }

    let verifications = chain["verification_records"].as_array();
    match verifications.map(Vec::as_slice).unwrap_or_default() {
        [] => println!("  checked   no Verification Record is retained"),
        records => {
            println!("  checked   {} Verification Record(s)", records.len());
            for record in records {
                let outcome = text(&record["outcome"]);
                let property = record["scope"]["property"]
                    .as_str()
                    .unwrap_or("property not recorded");
                println!("            {outcome} · {property}");
            }
        }
    }

    /* One lineage has one current authority chain. The event count says how
    much of that chain directly explains this Claim without resurrecting the
    retired compaction-era `standing_basis` vocabulary. */
    let events = chain["authority_events"].as_array().map_or(0, Vec::len);
    println!("  authority {events} event(s) directly explain this Claim");

    if let Some(by) = chain["supersession"]["successor_claim_id"].as_str() {
        println!("  superseded by {by}");
    }

    /* The three invariants the protocol will not let a reader lose. They are
    booleans in the payload; a person needs the sentence. */
    println!(
        "  a Submission is not an acceptance; a Verification Record reports one bounded check and is not an acceptance; Standing is derived from admitted Events, never read from a field."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_protocol::authority::{AUTHORITY_MODE, AuthorityEventContentV1};
    use vela_protocol::events::{EventKind, StateActor, StateTarget};

    #[test]
    fn byte_root_is_full_sha256() {
        assert_eq!(
            sha256_root(b"current"),
            "sha256:97b0560280ed60a5a1eaa1bc45492543c8a986ad5a25b468c427eb83c3e88191"
        );
    }

    /// A stored Verification Record answers nothing about its subject until it
    /// is opened.
    ///
    /// `why` filtered the stored JSON on `/subject/claim_id`, which was a
    /// field of the object before the DSSE cut and is now base64 inside the
    /// envelope. The pointer resolved to nothing for every record, so every
    /// Claim explained itself as having no Verification and no test noticed:
    /// an empty list is what a Claim with no Verification legitimately looks
    /// like. Whatever `verification_views` does, it must open the envelope
    /// first.
    #[test]
    fn a_stored_verification_record_hides_its_subject_until_the_envelope_is_opened() {
        const STORED: &[u8] =
            include_bytes!("../../../conformance/current-objects/verification.json");

        let raw: Value = serde_json::from_slice(STORED).expect("stored Verification Record JSON");
        assert_eq!(raw.pointer("/subject/claim_id"), None);

        let record = VerificationRecordEnvelopeV2::parse(STORED).expect("open the envelope");
        assert_eq!(
            record.record.subject.claim_id,
            "vcl_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
    }

    #[test]
    fn current_log_kind_filter_is_literal() {
        let filter = "review.";
        assert!("review.accepted".contains(filter));
        assert!(!"claim.asserted".contains(filter));
    }

    #[test]
    fn supersession_lookup_uses_covered_authority_history() {
        let event = AuthorityEventV1::new(AuthorityEventContentV1 {
            transaction_id: "vtx_fixture".into(),
            principal_id: "local:fixture|uid:501".into(),
            authority_mode: AUTHORITY_MODE.into(),
            kind: EventKind::ClaimSuperseded,
            target: StateTarget {
                r#type: "claim".into(),
                id: "vcl_predecessor".into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: "local:fixture|uid:501".into(),
            },
            timestamp: "2026-07-29T00:00:00Z".into(),
            reason: "Accept the exact replacement.".into(),
            before_hash: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                .into(),
            after_hash: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .into(),
            payload: json!({
                "claim_id": "vcl_successor",
                "proposal_id": "vpr_fixture",
            }),
            caveats: Vec::new(),
        })
        .unwrap();

        assert_eq!(
            supersession_event(&[event], "vcl_predecessor")
                .expect("supersession")
                .content
                .after_hash,
            "sha256:2222222222222222222222222222222222222222222222222222222222222222"
        );
    }
}
