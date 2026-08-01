//! Current object, standing, and authority-history readers.
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
use vela_protocol::current_repository::{
    ClaimStandingRefV1, CurrentRepositoryV4, RepositoryObjectRefV1,
};
use vela_protocol::proposal_v1::ProposalV1;
use vela_protocol::repository_origin::RepositoryOriginV1;

use crate::cli::{fail_return, print_json};
use crate::current_repository::CurrentProposalDecision;

struct CurrentReadContext {
    repository: CurrentRepositoryV4,
    repository_root: String,
    proposals: Vec<(RepositoryObjectRefV1, ProposalV1)>,
    decisions: BTreeMap<String, CurrentProposalDecision>,
    authority_events: Vec<AuthorityEventV1>,
}

fn root_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn canonical_root<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    vela_protocol::canonical::sha256_canonical(value).map(|digest| format!("sha256:{digest}"))
}

fn read_exact(frontier: &Path, path: &str, expected_root: &str) -> Result<Vec<u8>, String> {
    let candidate = frontier.join(path);
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|error| format!("inspect current object {path}: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("current object {path} must be a regular file"));
    }
    let bytes =
        fs::read(&candidate).map_err(|error| format!("read current object {path}: {error}"))?;
    if root_bytes(&bytes) != expected_root {
        return Err(format!(
            "current object {path} does not match {expected_root}"
        ));
    }
    Ok(bytes)
}

fn read_value(frontier: &Path, reference: &RepositoryObjectRefV1) -> Result<Value, String> {
    let bytes = read_exact(frontier, &reference.path, &reference.root)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse current object {}: {error}", reference.path))
}

fn load_context(frontier: &Path) -> Result<CurrentReadContext, String> {
    let repository = crate::current_repository::load_current_repository_at(frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let origin_bytes = fs::read(frontier.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    let authority = crate::cli::load_current_repository_authority(frontier, &repository, &origin)?;
    let decisions =
        crate::current_repository::load_current_proposal_decisions(frontier, &repository)?;
    let proposals = repository
        .proposals
        .iter()
        .map(|reference| {
            let bytes = read_exact(frontier, &reference.path, &reference.root)?;
            let proposal = ProposalV1::parse(&bytes)?;
            if proposal.proposal_id != reference.id {
                return Err(format!(
                    "current Proposal {} does not match its repository reference",
                    reference.id
                ));
            }
            Ok((reference.clone(), proposal))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(CurrentReadContext {
        repository,
        repository_root,
        proposals,
        decisions,
        authority_events: authority.history.authority_events,
    })
}

fn claim_reference<'a>(
    context: &'a CurrentReadContext,
    claim_id: &str,
) -> Option<(&'a ClaimStandingRefV1, &'static str)> {
    context
        .repository
        .accepted_claims
        .iter()
        .find(|reference| reference.claim_id == claim_id)
        .map(|reference| (reference, "accepted"))
        .or_else(|| {
            context
                .repository
                .pending_claims
                .iter()
                .find(|reference| reference.claim_id == claim_id)
                .map(|reference| (reference, "pending_review"))
        })
}

fn proposal_claim(
    frontier: &Path,
    context: &CurrentReadContext,
    claim_id: &str,
) -> Result<Option<(ClaimRecordV1, String)>, String> {
    let mut matches = context
        .proposals
        .iter()
        .filter(|(_, proposal)| proposal.subject.id == claim_id)
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.1.created_at.cmp(&right.1.created_at));
    let Some((_, proposal)) = matches.last() else {
        return Ok(None);
    };
    let path =
        crate::current_submission::rooted_path("records/claims/sha256", &proposal.subject.root)?;
    let claim = ClaimRecordV1::parse(&read_exact(frontier, &path, &proposal.subject.root)?)?;
    if claim.claim_id != proposal.subject.id {
        return Err(format!(
            "current Proposal {} resolves to the wrong Claim",
            proposal.proposal_id
        ));
    }
    let standing = context
        .decisions
        .get(&proposal.proposal_id)
        .map(|decision| decision.standing.clone())
        .unwrap_or_else(|| "pending_review".into());
    Ok(Some((claim, standing)))
}

fn supersession_event<'a>(
    authority_events: &'a [AuthorityEventV1],
    claim_id: &str,
) -> Option<&'a AuthorityEventV1> {
    authority_events.iter().rev().find(|event| {
        event.content.kind.as_str() == "finding.superseded"
            && event.content.target.r#type == "claim"
            && event.content.target.id == claim_id
    })
}

fn superseded_claim(
    frontier: &Path,
    context: &CurrentReadContext,
    claim_id: &str,
) -> Result<Option<(ClaimRecordV1, String, String)>, String> {
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
    let path = crate::current_submission::rooted_path("records/claims/sha256", &root)?;
    let claim = ClaimRecordV1::parse(&read_exact(frontier, &path, &root)?)?;
    if claim.claim_id != claim_id {
        return Err(format!(
            "supersession event {} resolves to the wrong predecessor Claim",
            event.id
        ));
    }
    Ok(Some((claim, root, "superseded".into())))
}

fn load_claim(
    frontier: &Path,
    context: &CurrentReadContext,
    claim_id: &str,
) -> Result<(ClaimRecordV1, String, String), String> {
    if let Some((reference, standing)) = claim_reference(context, claim_id) {
        let claim = ClaimRecordV1::parse(&read_exact(
            frontier,
            &reference.path,
            &reference.claim_root,
        )?)?;
        return Ok((claim, reference.claim_root.clone(), standing.into()));
    }
    if let Some((claim, standing)) = proposal_claim(frontier, context, claim_id)? {
        let root = canonical_root(&claim)?;
        return Ok((claim, root, standing));
    }
    if let Some(claim) = superseded_claim(frontier, context, claim_id)? {
        return Ok(claim);
    }
    Err(format!(
        "no current or retained superseded Claim '{claim_id}' in this frontier"
    ))
}

fn supersession_view(
    context: &CurrentReadContext,
    claim_id: &str,
) -> Result<Option<Value>, String> {
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

fn proposal_views(context: &CurrentReadContext, claim_id: &str) -> Vec<Value> {
    context
        .proposals
        .iter()
        .filter(|(_, proposal)| proposal.subject.id == claim_id)
        .map(|(reference, proposal)| {
            json!({
                "proposal": proposal,
                "proposal_root": reference.root,
                "decision": context.decisions.get(&proposal.proposal_id),
            })
        })
        .collect()
}

fn verification_views(
    frontier: &Path,
    context: &CurrentReadContext,
    claim_id: &str,
) -> Result<Vec<Value>, String> {
    context
        .repository
        .verifications
        .iter()
        .map(|reference| read_value(frontier, reference))
        .filter_map(|result| match result {
            Ok(value)
                if value.pointer("/subject/claim_id").and_then(Value::as_str) == Some(claim_id) =>
            {
                Some(Ok(value))
            }
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

fn related_authority_events(
    context: &CurrentReadContext,
    claim_id: &str,
) -> Result<Vec<Value>, String> {
    let proposal_ids = context
        .proposals
        .iter()
        .filter(|(_, proposal)| proposal.subject.id == claim_id)
        .map(|(_, proposal)| proposal.proposal_id.as_str())
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
                    .is_some_and(|proposal_id| proposal_ids.contains(&proposal_id))
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
    context: &CurrentReadContext,
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
        "frontier_id": context.repository.frontier_id,
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

pub(crate) fn show_payload(frontier: &Path, object_id: &str) -> Result<Value, String> {
    let context = load_context(frontier)?;
    if let Some((reference, standing)) = claim_reference(&context, object_id) {
        return Ok(object_projection(
            &context,
            object_id,
            "claim",
            vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA,
            &format!("scientific standing is {standing}, derived from current authority"),
            reference.claim_root.clone(),
            serde_json::from_slice(&read_exact(
                frontier,
                &reference.path,
                &reference.claim_root,
            )?)
            .map_err(|error| format!("parse current Claim: {error}"))?,
        ));
    }
    if object_id.starts_with("vcl_")
        && let Ok((claim, root, standing)) = load_claim(frontier, &context, object_id)
    {
        return Ok(object_projection(
            &context,
            object_id,
            "claim",
            vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA,
            &format!("scientific standing is {standing}, derived from current authority"),
            root,
            serde_json::to_value(claim).map_err(|error| error.to_string())?,
        ));
    }
    if let Some((reference, proposal)) = context
        .proposals
        .iter()
        .find(|(_, proposal)| proposal.proposal_id == object_id)
    {
        return Ok(object_projection(
            &context,
            object_id,
            "proposal",
            vela_protocol::proposal_v1::PROPOSAL_V1_SCHEMA,
            "requests a scientific-state change; standing is derived from an authorized Decision",
            reference.root.clone(),
            json!({
                "proposal": proposal,
                "decision": context.decisions.get(object_id),
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
                read_value(frontier, reference)?,
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
    Err(format!(
        "no exact current object '{object_id}' in this frontier"
    ))
}

pub(crate) fn why_payload(frontier: &Path, claim_id: &str) -> Result<Value, String> {
    let context = load_context(frontier)?;
    let (claim, claim_root, standing) = load_claim(frontier, &context, claim_id)?;
    Ok(json!({
        "ok": true,
        "command": "why",
        "schema": "vela.standing-explanation.v1",
        "frontier_id": context.repository.frontier_id,
        "repository_root": context.repository_root,
        "claim_id": claim_id,
        "claim_root": claim_root,
        "standing": standing,
        "chain": {
            "claim": claim,
            "proposals": proposal_views(&context, claim_id),
            "verification_records": verification_views(frontier, &context, claim_id)?,
            "authority_events": related_authority_events(&context, claim_id)?,
            "supersession": supersession_view(&context, claim_id)?,
        },
        "interpretation": {
            "submission_is_acceptance": false,
            "verification_is_acceptance": false,
            "standing_is_derived": true,
        },
    }))
}

pub(crate) fn log_payload(
    frontier: &Path,
    object_id: Option<&str>,
    limit: usize,
    kind_filter: Option<&str>,
    as_of: Option<&str>,
) -> Result<Value, String> {
    let context = load_context(frontier)?;
    let as_of = as_of
        .map(|value| {
            chrono::DateTime::parse_from_rfc3339(value)
                .map_err(|error| format!("invalid --as-of timestamp {value:?}: {error}"))
        })
        .transpose()?;
    let proposal_ids = object_id
        .map(|object_id| {
            context
                .proposals
                .iter()
                .filter(|(_, proposal)| {
                    proposal.proposal_id == object_id || proposal.subject.id == object_id
                })
                .map(|(_, proposal)| proposal.proposal_id.as_str())
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
                            proposal_id == object_id || proposal_ids.contains(&proposal_id)
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
        "frontier_id": context.repository.frontier_id,
        "repository_root": context.repository_root,
        "source_era": "current",
        "object_id": object_id,
        "as_of": as_of.map(|value| value.to_rfc3339()),
        "events": events,
    }))
}

pub(crate) fn cmd_show(frontier: &Path, object_id: &str, json_out: bool) {
    crate::ui::set_mode("show", json_out);
    let projection = show_payload(frontier, object_id).unwrap_or_else(|error| fail_return(&error));
    if json_out {
        print_json(&projection);
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&projection).expect("serialize current object projection")
        );
    }
}

pub(crate) fn cmd_why(frontier: &Path, claim_id: &str, json_out: bool) {
    crate::ui::set_mode("why", json_out);
    if !claim_id.starts_with("vcl_") {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Usage,
            "why requires a full Claim id",
            Some("use `vela why <frontier> vcl_... --json`"),
        );
    }
    let projection = why_payload(frontier, claim_id).unwrap_or_else(|error| fail_return(&error));
    if json_out {
        print_json(&projection);
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&projection)
                .expect("serialize current standing explanation")
        );
    }
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

    #[test]
    fn current_log_kind_filter_is_literal() {
        let filter = "review.";
        assert!("review.accepted".contains(filter));
        assert!(!"finding.asserted".contains(filter));
    }

    #[test]
    fn supersession_lookup_uses_covered_authority_history() {
        let event = AuthorityEventV1::new(AuthorityEventContentV1 {
            transaction_id: "vtx_fixture".into(),
            principal_id: "local:fixture|uid:501".into(),
            authority_mode: AUTHORITY_MODE.into(),
            kind: EventKind::FindingSuperseded,
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
