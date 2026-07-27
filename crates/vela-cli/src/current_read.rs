//! Current-epoch object, standing, and authority-history readers.
//!
//! A repository epoch retires the Era-0 project snapshot as an active read
//! dependency. These projections therefore use only the verified current
//! repository manifest, its content-addressed records, and covered repository
//! authority history.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_protocol::authority::AuthorityEventV1;
use vela_protocol::claim_record::ClaimRecordV1;
use vela_protocol::current_repository::{
    ClaimStandingRefV1, CurrentRepositoryV2, RepositoryObjectRefV1,
};
use vela_protocol::proposal_v1::ProposalV1;
use vela_protocol::repository_epoch::RepositoryEpochV1;

use crate::repository_upgrade::CurrentProposalDecision;

struct CurrentReadContext {
    repository: CurrentRepositoryV2,
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
    let repository = crate::repository_upgrade::verify_current_repository_at(frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let epoch_bytes = fs::read(frontier.join(".vela/epoch.json"))
        .map_err(|error| format!("read current repository epoch: {error}"))?;
    let epoch = RepositoryEpochV1::parse(&epoch_bytes)?;
    let authority = crate::cli::load_current_repository_authority(frontier, &repository, &epoch)?;
    let decisions =
        crate::repository_upgrade::load_current_proposal_decisions(frontier, &repository)?;
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
    Err(format!("no current Claim '{claim_id}' in this frontier"))
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
            &context.repository.registrations,
            "registration_record",
            "intake provenance; no accepted-state authority",
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

pub(crate) fn claim_payload(frontier: &Path, claim_id: &str, view: &str) -> Result<Value, String> {
    let context = load_context(frontier)?;
    let (claim, claim_root, standing) = load_claim(frontier, &context, claim_id)?;
    let proposals = proposal_views(&context, claim_id);
    let payload = match view {
        "record" => json!({
            "ok": true,
            "command": "claim.show",
            "schema": "vela.claim-view.v1",
            "view": "record",
            "frontier_id": context.repository.frontier_id,
            "repository_root": context.repository_root,
            "claim_id": claim_id,
            "claim_root": claim_root,
            "source_era": "current",
            "source_schema": claim.schema,
            "record": claim,
        }),
        "standing" => json!({
            "ok": true,
            "command": "claim.show",
            "schema": "vela.claim-view.v1",
            "view": "standing",
            "frontier_id": context.repository.frontier_id,
            "repository_root": context.repository_root,
            "claim_id": claim_id,
            "claim_root": claim_root,
            "source_era": "current",
            "standing": standing,
            "proposals": proposals,
        }),
        "evidence" => json!({
            "ok": true,
            "command": "claim.show",
            "schema": "vela.claim-view.v1",
            "view": "evidence",
            "frontier_id": context.repository.frontier_id,
            "repository_root": context.repository_root,
            "claim_id": claim_id,
            "claim_root": claim_root,
            "source_era": "current",
            "evidence": claim.evidence,
            "verification_records": verification_views(frontier, &context, claim_id)?,
        }),
        "attribution" => json!({
            "ok": true,
            "command": "claim.show",
            "schema": "vela.claim-view.v1",
            "view": "attribution",
            "frontier_id": context.repository.frontier_id,
            "repository_root": context.repository_root,
            "claim_id": claim_id,
            "claim_root": claim_root,
            "source_era": "current",
            "created_at": claim.created_at,
            "provenance": claim.provenance,
            "imported_from": claim.imported_from,
        }),
        other => return Err(format!("unsupported Claim view {other:?}")),
    };
    Ok(payload)
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
        },
        "interpretation": {
            "submission_is_acceptance": false,
            "verification_is_acceptance": false,
            "standing_is_derived": true,
            "legacy_runtime_used": false,
        },
    }))
}

pub(crate) fn log_payload(
    frontier: &Path,
    limit: usize,
    kind_filter: Option<&str>,
) -> Result<Value, String> {
    let context = load_context(frontier)?;
    let mut events = context
        .authority_events
        .iter()
        .filter(|event| {
            kind_filter.is_none_or(|filter| event.content.kind.as_str().contains(filter))
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
        "legacy_runtime_used": false,
        "events": events,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
