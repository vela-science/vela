//! Object, standing, and authority-history readers.
//!
//! These projections use only the verified repository manifest, its
//! content-addressed records, and covered repository-authority history.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_protocol::authority::AuthorityEventV1;
use vela_protocol::claim_record::ClaimRecordV1;
use vela_protocol::proposal::ProposalV1;
use vela_protocol::proposal_withdrawal::ProposalWithdrawalEnvelopeV2;
use vela_protocol::repository::{ClaimStandingRefV1, RepositoryObjectRefV1, RepositoryV4};
use vela_protocol::repository_origin::RepositoryOriginV1;
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
}

fn root_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
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
    if root_bytes(&bytes) != expected_root {
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
    Ok(ReadContext {
        repository,
        repository_root,
        origin,
        proposals,
        decisions,
        withdrawals,
        authority_events: authority.history.authority_events,
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
    if let Some((claim, root)) = superseded_claim(repository_path, context, claim_id)? {
        let standing = ClaimStanding {
            standing: claim_standing::SUPERSEDED,
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
        answers "does this Claim stand?" in the vocabulary TERMINOLOGY.md
        declares; `proposal_status` answers "what happened to the Proposal about
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

    /* standing_basis distinguishes a Claim carried through compaction from one
    decided in the current authority chain. Print the count of current-chain
    events beside it: where a Claim reports `compacted_origin` and still has
    events here, the two disagree and the reader should see both. */
    let basis = text(&chain["standing_basis"]);
    let events = chain["authority_events"].as_array().map_or(0, Vec::len);
    println!("  basis     {basis} · {events} event(s) in the current authority chain");

    if let Some(by) = chain["supersession"]["superseded_by"].as_str() {
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
            root_bytes(b"current"),
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
