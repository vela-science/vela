//! Object-first read surfaces: `vela show` and root-bound `vela why`.

use crate::cli::{fail_return, print_json};
use serde::Serialize;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use vela_protocol::{events, proposals, repo};

fn canonical_root<T: Serialize + ?Sized>(value: &T) -> String {
    format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(value)
            .unwrap_or_else(|error| fail_return(&format!("canonicalize object: {error}")))
    )
}

fn object_projection(
    frontier_id: &str,
    object_id: &str,
    object_kind: &str,
    object_schema: &str,
    source_era: &str,
    authority_effect: &str,
    object: Value,
) -> Value {
    json!({
        "ok": true,
        "command": "show",
        "schema": "vela.object-view.v1",
        "frontier_id": frontier_id,
        "object_id": object_id,
        "object_kind": object_kind,
        "object_schema": object_schema,
        "source_era": source_era,
        "content_root": canonical_root(&object),
        "authority_effect": authority_effect,
        "object": object,
    })
}

pub(crate) fn candidate_files(frontier: &Path, directory: &str) -> Result<Vec<PathBuf>, String> {
    let root = frontier.join(directory);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let metadata = fs::symlink_metadata(&root)
        .map_err(|error| format!("inspect {}: {error}", root.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "{} must be a real repository directory",
            root.display()
        ));
    }
    let mut files = fs::read_dir(&root)
        .map_err(|error| format!("read {}: {error}", root.display()))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| format!("read {} entry: {error}", root.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    Ok(files)
}

fn read_current_record(frontier: &Path, object_id: &str) -> Result<Option<Value>, String> {
    let (directory, object_kind, object_schema, authority_effect) = if object_id.starts_with("vsb_")
    {
        (
            "records/submissions/sha256",
            "submission",
            vela_protocol::submission_v1::SUBMISSION_V1_SCHEMA,
            "authenticated producer input; no accepted-state authority",
        )
    } else if object_id.starts_with("vvr_") {
        (
            "records/verifications/sha256",
            "verification_record",
            vela_protocol::verification_record::VERIFICATION_RECORD_V1_SCHEMA,
            "authenticated verification observation; no accepted-state authority",
        )
    } else if object_id.starts_with("vrr_") {
        (
            "records/registrations/sha256",
            "registration_record",
            vela_protocol::registration_record::REGISTRATION_RECORD_V1_SCHEMA,
            "Vela intake provenance; no accepted-state authority",
        )
    } else {
        return Ok(None);
    };
    for path in candidate_files(frontier, directory)? {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("inspect {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let bytes = crate::bounded_file::read_bounded_file(&path, 8 * 1024 * 1024, object_kind)
            .map_err(|error| error.to_string())?;
        let value = if object_kind == "submission" {
            serde_json::to_value(vela_protocol::submission_v1::SubmissionV1::parse(&bytes)?)
                .map_err(|error| error.to_string())?
        } else if object_kind == "verification_record" {
            serde_json::to_value(
                vela_protocol::verification_record::VerificationRecordV1::parse(&bytes)?,
            )
            .map_err(|error| error.to_string())?
        } else {
            serde_json::to_value(
                vela_protocol::registration_record::RegistrationRecordV1::parse(&bytes)?,
            )
            .map_err(|error| error.to_string())?
        };
        let observed_id = value
            .get(if object_kind == "submission" {
                "submission_id"
            } else if object_kind == "verification_record" {
                "verification_record_id"
            } else {
                "registration_record_id"
            })
            .and_then(Value::as_str);
        if observed_id == Some(object_id) {
            return Ok(Some(json!({
                "object_kind": object_kind,
                "object_schema": object_schema,
                "authority_effect": authority_effect,
                "object": value,
            })));
        }
    }
    Ok(None)
}

pub(crate) fn cmd_show(frontier: &Path, object_id: &str, json_out: bool) {
    crate::ui::set_mode("show", json_out);
    let project = repo::load_from_path(frontier).unwrap_or_else(|error| fail_return(&error));
    let frontier_id = project.frontier_id();
    let projection = if let Some(record) =
        read_current_record(frontier, object_id).unwrap_or_else(|error| fail_return(&error))
    {
        object_projection(
            &frontier_id,
            object_id,
            record["object_kind"].as_str().unwrap_or("record"),
            record["object_schema"].as_str().unwrap_or("unknown"),
            "current",
            record["authority_effect"].as_str().unwrap_or("none"),
            record["object"].clone(),
        )
    } else if let Some(proposal) = project
        .proposals
        .iter()
        .find(|proposal| proposal.id == object_id)
    {
        object_projection(
            &frontier_id,
            object_id,
            "proposal",
            "vela.state-proposal.v1",
            if proposal.payload.get("submission").is_some() {
                "current"
            } else {
                "historical"
            },
            "requests a scientific-state change; standing changes only through an authorized Decision",
            serde_json::to_value(proposal).expect("Proposal is serializable"),
        )
    } else if let Some(finding) = project
        .findings
        .iter()
        .find(|finding| finding.id == object_id)
    {
        object_projection(
            &frontier_id,
            object_id,
            "finding",
            "vela.finding-bundle.v1",
            "historical",
            "accepted scientific-state record",
            serde_json::to_value(finding).expect("Finding is serializable"),
        )
    } else if let Some(event) = project.events.iter().find(|event| event.id == object_id) {
        object_projection(
            &frontier_id,
            object_id,
            "event",
            "vela.state-event.v1",
            "historical",
            "canonical authority event; effect is determined by its kind and valid signature",
            serde_json::to_value(event).expect("Event is serializable"),
        )
    } else if let Some(artifact) = project
        .artifacts
        .iter()
        .find(|artifact| artifact.id == object_id)
    {
        object_projection(
            &frontier_id,
            object_id,
            "artifact",
            "vela.artifact.v1",
            "historical",
            "content provenance only; not verification or acceptance",
            serde_json::to_value(artifact).expect("Artifact is serializable"),
        )
    } else if let Some(attachment) = project
        .verifier_attachments
        .iter()
        .find(|attachment| attachment.id == object_id)
    {
        object_projection(
            &frontier_id,
            object_id,
            "verifier_attachment",
            "vela.verifier-attachment.v1",
            "historical",
            "historical verification evidence; no accepted-state authority",
            serde_json::to_value(attachment).expect("Verifier Attachment is serializable"),
        )
    } else {
        crate::ui::fail_with(
            crate::ui::ErrorKind::NotFound,
            &format!("no exact object '{object_id}' in this frontier"),
            Some("use a full stable id; list proposals with `vela review list`"),
        )
    };
    if json_out {
        print_json(&projection);
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&projection).expect("serialize object projection")
        );
    }
}

pub(crate) fn verification_records_for_proposal(
    frontier: &Path,
    proposal_id: &str,
) -> Result<
    Vec<(
        String,
        vela_protocol::verification_record::VerificationRecordV1,
    )>,
    String,
> {
    let mut records = Vec::new();
    for path in candidate_files(frontier, "records/verifications/sha256")? {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("inspect {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let bytes =
            crate::bounded_file::read_bounded_file(&path, 4 * 1024 * 1024, "Verification Record")
                .map_err(|error| error.to_string())?;
        let record = vela_protocol::verification_record::VerificationRecordV1::parse(&bytes)?;
        if record.subject.proposal_id == proposal_id {
            let relative = path
                .strip_prefix(frontier)
                .map_err(|_| format!("{} is outside the frontier", path.display()))?
                .to_str()
                .ok_or_else(|| format!("{} is not UTF-8", path.display()))?
                .to_string();
            records.push((relative, record));
        }
    }
    records.sort_by(|left, right| {
        left.1
            .verification_record_id
            .cmp(&right.1.verification_record_id)
    });
    Ok(records)
}

pub(crate) fn verification_records_for_claim(
    frontier: &Path,
    claim_id: &str,
) -> Result<Vec<Value>, String> {
    let mut records = Vec::new();
    for path in candidate_files(frontier, "records/verifications/sha256")? {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("inspect {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let bytes =
            crate::bounded_file::read_bounded_file(&path, 4 * 1024 * 1024, "Verification Record")
                .map_err(|error| error.to_string())?;
        let record = vela_protocol::verification_record::VerificationRecordV1::parse(&bytes)?;
        if record.subject.claim_id == claim_id {
            records.push(serde_json::to_value(record).map_err(|error| error.to_string())?);
        }
    }
    Ok(records)
}

pub(crate) fn cmd_why(frontier: &Path, claim_id: &str, json_out: bool) {
    crate::ui::set_mode("why", json_out);
    if !(claim_id.starts_with("vf_") || claim_id.starts_with("vcl_")) {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Usage,
            "why requires a full Claim or historical Finding id",
            Some("use `vela why <frontier> vf_... --json`"),
        );
    }
    let project = repo::load_from_path(frontier).unwrap_or_else(|error| fail_return(&error));
    let finding = project
        .findings
        .iter()
        .find(|finding| finding.id == claim_id);
    if claim_id.starts_with("vf_") && finding.is_none() {
        crate::ui::fail_with(
            crate::ui::ErrorKind::NotFound,
            &format!("no historical Finding '{claim_id}' in this frontier"),
            None,
        );
    }
    let related_proposals = project
        .proposals
        .iter()
        .filter(|proposal| proposal.target.id == claim_id)
        .collect::<Vec<_>>();
    let current_verifications = verification_records_for_claim(frontier, claim_id)
        .unwrap_or_else(|error| fail_return(&error));
    let historical_verifications = project
        .verifier_attachments
        .iter()
        .filter(|attachment| attachment.target == claim_id)
        .collect::<Vec<_>>();
    let related_events = project
        .events
        .iter()
        .filter(|event| {
            event.target.id == claim_id
                || related_proposals.iter().any(|proposal| {
                    event.payload.get("proposal_id").and_then(Value::as_str)
                        == Some(proposal.id.as_str())
                })
        })
        .collect::<Vec<_>>();
    let standing = finding
        .map(|finding| {
            json!({
                "accepted": true,
                "review_state": finding.flags.review_state,
                "retracted": finding.flags.retracted,
                "contested": finding.flags.contested,
                "superseded": finding.flags.superseded,
            })
        })
        .unwrap_or_else(|| {
            let pending = related_proposals
                .iter()
                .any(|proposal| proposal.status == "pending_review");
            json!({
                "accepted": false,
                "proposal_pending": pending,
                "review_state": if pending { "pending_review" } else { "unregistered" },
            })
        });
    let projection = json!({
        "ok": true,
        "command": "why",
        "schema": "vela.standing-explanation.v1",
        "frontier_id": project.frontier_id(),
        "claim_id": claim_id,
        "standing": standing,
        "chain": {
            "claim": finding,
            "proposals": related_proposals,
            "verification_records": current_verifications,
            "historical_verifier_attachments": historical_verifications,
            "authority_events": related_events,
        },
        "roots": {
            "event_log": format!("sha256:{}", events::event_log_hash(&project.events)),
            "proposals": format!("sha256:{}", proposals::proposal_state_hash(&project.proposals)),
            "claim": finding.map(canonical_root),
        },
        "interpretation": {
            "submission_is_acceptance": false,
            "verification_is_acceptance": false,
            "standing_is_derived": true,
        },
    });
    if json_out {
        print_json(&projection);
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&projection).expect("serialize standing explanation")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::submission_v1::{
        RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft,
        SubmissionProvenance, SubmissionV1,
    };

    #[test]
    fn current_submission_show_lookup_verifies_exact_bytes() {
        let directory = TempDir::new().unwrap();
        let key = SigningKey::from_bytes(&[81_u8; 32]);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:object-show".into(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-26T00:00:00Z".into(),
            },
            &key,
        )
        .unwrap();
        let submission = SubmissionV1::build(
            SubmissionDraft {
                claim: SubmissionClaim {
                    assertion: "One exact object is inspectable.".into(),
                    claim_type: "computational".into(),
                    conditions: Vec::new(),
                },
                artifacts: vec![SubmissionArtifact {
                    kind: "witness".into(),
                    path: "witness.json".into(),
                    digest: format!("sha256:{}", "a".repeat(64)),
                }],
                caveats: vec!["Inspection is not acceptance.".into()],
                replayability: "exact".into(),
                producer_checks: Vec::new(),
                verification_requirements: vec!["independent replay".into()],
                requested_change: RequestedChange {
                    kind: "add_claim".into(),
                    target: None,
                },
                provenance: SubmissionProvenance {
                    producer: "agent:object-show".into(),
                    source_system: "fixture".into(),
                    source_attempt: None,
                    source_run: None,
                    emitted_at: "2026-07-26T00:00:00Z".into(),
                },
                execution_binding: None,
            },
            identity,
            &key,
        )
        .unwrap();
        let root = submission.canonical_root().unwrap();
        let path = directory.path().join(format!(
            "records/submissions/sha256/{}.json",
            root.strip_prefix("sha256:").unwrap()
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, submission.canonical_bytes().unwrap()).unwrap();

        let found = read_current_record(directory.path(), &submission.submission_id)
            .unwrap()
            .unwrap();
        assert_eq!(found["object_kind"], "submission");
        assert_eq!(
            found["object"]["submission_id"],
            submission.submission_id.as_str()
        );

        let mut tampered: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        tampered["claim"]["assertion"] = json!("tampered");
        fs::write(&path, serde_json::to_vec(&tampered).unwrap()).unwrap();
        assert!(read_current_record(directory.path(), &submission.submission_id).is_err());
    }
}
