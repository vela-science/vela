//! Current evidence authoring, publication, and repository-operation helpers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use vela_protocol::submission_v1::{
    ProducerCheck, RequestedChange, RequestedChangeTarget, SubmissionArtifact, SubmissionClaim,
    SubmissionDraft, SubmissionProvenance, SubmissionV1,
};

pub(crate) fn submission_requested_change(
    corrects: Option<String>,
    supersedes: Option<String>,
    target_root: Option<String>,
) -> Result<RequestedChange, String> {
    match (corrects, supersedes, target_root) {
        (None, None, None) => Ok(RequestedChange {
            kind: "add_claim".to_string(),
            target: None,
        }),
        (Some(claim_id), None, Some(claim_root)) => Ok(RequestedChange {
            kind: "correct_claim".to_string(),
            target: Some(RequestedChangeTarget {
                claim_id,
                claim_root,
            }),
        }),
        (None, Some(claim_id), Some(claim_root)) => Ok(RequestedChange {
            kind: "supersede_claim".to_string(),
            target: Some(RequestedChangeTarget {
                claim_id,
                claim_root,
            }),
        }),
        (Some(_), Some(_), _) => {
            Err("--corrects and --supersedes are mutually exclusive".to_string())
        }
        (Some(_), None, None) | (None, Some(_), None) => {
            Err("--corrects and --supersedes require --target-root".to_string())
        }
        (None, None, Some(_)) => {
            Err("--target-root requires --corrects or --supersedes".to_string())
        }
    }
}

pub(crate) fn active_repository_signing_key(
    authority: &crate::cli::LoadedRepositoryAuthority,
) -> Result<(String, String), String> {
    let sequence = u64::try_from(authority.verification.authority_record_count + 1)
        .map_err(|_| "repository-authority sequence exceeds u64".to_string())?;
    if authority.history.authority_keyset.threshold != 1 {
        return Err(
            "routine local repository-authority writes currently require a one-key threshold"
                .into(),
        );
    }
    let active = authority
        .history
        .authority_keyset
        .keys
        .iter()
        .filter(|key| {
            key.valid_from_sequence <= sequence
                && key
                    .valid_through_sequence
                    .is_none_or(|through| sequence <= through)
        })
        .collect::<Vec<_>>();
    let [key] = active.as_slice() else {
        return Err(format!(
            "routine local repository-authority writes require exactly one active key at sequence {sequence}; found {}",
            active.len()
        ));
    };
    Ok((key.key_id.clone(), key.public_key.clone()))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn author_submission(
    frontier: &Path,
    actor: &str,
    assertion: String,
    claim_type: String,
    conditions: Vec<String>,
    replayability: String,
    artifact_flags: &[String],
    caveats: Vec<String>,
    producer_checks: Vec<String>,
    verification_requirements: Vec<String>,
    requested_change: RequestedChange,
    execution_binding: Option<vela_protocol::execution_binding::ExecutionBindingV1>,
) -> Result<SubmissionV1, String> {
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};

    if !(actor.starts_with("agent:") || actor.starts_with("ci:")) {
        return Err("Submission authoring requires an agent: or ci: producer".to_string());
    }
    let emitted_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut artifacts = Vec::new();
    let mut total_artifact_bytes = 0_u64;
    for (index, flag) in artifact_flags.iter().enumerate() {
        let (path, kind) = if frontier.join(flag).is_file() {
            (flag.as_str(), "other")
        } else {
            flag.rsplit_once(':').unwrap_or((flag.as_str(), "other"))
        };
        let relative = Path::new(path);
        if relative.is_absolute()
            || !relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)))
        {
            return Err(format!(
                "artifact {index} must be a normalized frontier-relative file"
            ));
        }
        let read_limit = public_artifact_read_limit(total_artifact_bytes, index)?;
        let bytes = crate::bounded_file::read_bounded_frontier_file(
            frontier,
            relative,
            read_limit,
            &format!("artifact {index}"),
        )
        .map_err(|error| public_artifact_read_error(error, read_limit, index))?;
        account_public_artifact_bytes(&mut total_artifact_bytes, bytes.len() as u64, index)?;
        artifacts.push(SubmissionArtifact {
            kind: kind.to_string(),
            path: path.to_string(),
            digest: format!("sha256:{}", hex::encode(Sha256::digest(&bytes))),
        });
    }
    let checks = producer_checks
        .into_iter()
        .map(|value| {
            let (method, outcome) = value.rsplit_once(':').ok_or_else(|| {
                "producer checks use --check <method>:<pass|fail|error|skipped|unknown>".to_string()
            })?;
            ProducerCheck::new(method.to_string(), outcome.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let key = vela_edge::agent_identity::agent_signing_key(Some(actor))?;
    let identity = IdentityBinding::build(
        IdentityBindingDraft {
            actor_id: actor.to_string(),
            actor_class: ActorClass::Agent,
            created_at: emitted_at.clone(),
        },
        &key,
    )?;
    SubmissionV1::build(
        SubmissionDraft {
            claim: SubmissionClaim {
                assertion,
                claim_type,
                conditions,
            },
            artifacts,
            caveats,
            replayability,
            producer_checks: checks,
            verification_requirements,
            requested_change,
            provenance: SubmissionProvenance {
                producer: actor.to_string(),
                source_system: "vela-cli".to_string(),
                source_attempt: None,
                source_run: None,
                emitted_at,
            },
            execution_binding,
        },
        identity,
        &key,
    )
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubmitOutcome {
    pub schema: &'static str,
    pub operation_id: String,
    pub submission_id: String,
    pub submission_root: String,
    pub proposal_id: String,
    pub claim_id: String,
    pub route: &'static str,
    pub accepted_event_count_before: usize,
    pub accepted_event_count_after: usize,
    pub accepted_event_delta: usize,
    pub accepted_state_changed: bool,
    pub publication: crate::config::git_publish::PublicationOutcome,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct VerificationImportOutcome {
    pub schema: &'static str,
    pub operation_id: String,
    pub verification_record_id: String,
    pub verification_record_root: String,
    pub proposal_id: String,
    pub claim_id: String,
    pub outcome: String,
    pub idempotent: bool,
    pub accepted_event_delta: usize,
    pub publication: crate::config::git_publish::PublicationOutcome,
}

#[derive(Debug)]
pub(crate) struct PreparedSubmissionArtifacts {
    pub(crate) writes: Vec<crate::frontier_txn::PlannedWrite>,
    pub(crate) read_set: Vec<crate::frontier_txn::InputBinding>,
}

pub(crate) fn prepare_submission_artifacts(
    frontier: &Path,
    submission: &SubmissionV1,
    bundle_root: Option<&Path>,
) -> Result<PreparedSubmissionArtifacts, String> {
    use crate::frontier_txn::{ContentDigest, InputBinding, PlannedWrite, RepoPath, WriteClass};

    let mut blobs = BTreeMap::<String, Vec<u8>>::new();
    let mut read_set = Vec::new();
    let mut total = 0_u64;
    for (index, artifact) in submission.artifacts.iter().enumerate() {
        let relative = Path::new(&artifact.path);
        if relative.is_absolute()
            || !relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)))
        {
            return Err(format!(
                "Submission artifact {index} must be a normalized frontier-relative file"
            ));
        }
        let limit = public_artifact_read_limit(total, index)?;
        let declared_hex = artifact
            .digest
            .strip_prefix("sha256:")
            .ok_or_else(|| format!("Submission artifact {index} digest is not sha256"))?;
        let canonical_path = format!("records/artifacts/sha256/{declared_hex}");
        let canonical_relative = Path::new(&canonical_path);
        let canonical_target = frontier.join(canonical_relative);
        let bytes = if canonical_target.exists() {
            let bytes = crate::bounded_file::read_bounded_frontier_file(
                frontier,
                canonical_relative,
                limit,
                &format!("Submission artifact {index}"),
            )
            .map_err(|error| public_artifact_read_error(error, limit, index))?;
            let tracked = std::process::Command::new("git")
                .current_dir(frontier)
                .args(["ls-files", "--error-unmatch", "--", &canonical_path])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map_err(|error| format!("inspect canonical Artifact tracking: {error}"))?
                .success();
            if !tracked {
                return Err(format!(
                    "Submission artifact {index} already occupies its canonical path but is untracked; remove it and keep the transport blob beside submission.json under artifacts/sha256/{declared_hex}"
                ));
            }
            bytes
        } else if relative == canonical_relative {
            let root = bundle_root.ok_or_else(|| {
                format!(
                    "Submission artifact {index} is absent; place its transport blob beside submission.json under artifacts/sha256/{declared_hex}"
                )
            })?;
            let canonical_root = root
                .canonicalize()
                .map_err(|error| format!("canonicalize Submission transport root: {error}"))?;
            let transport_directory = canonical_root.join("artifacts").join("sha256");
            let source = transport_directory.join(declared_hex);
            let metadata = std::fs::symlink_metadata(&source).map_err(|error| {
                format!("inspect Submission transport artifact {index}: {error}")
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "Submission transport artifact {index} must be a regular non-symlink file"
                ));
            }
            let canonical_source = source.canonicalize().map_err(|error| {
                format!("canonicalize Submission transport artifact {index}: {error}")
            })?;
            if canonical_source != source || !canonical_source.starts_with(&transport_directory) {
                return Err(format!(
                    "Submission transport artifact {index} escapes its canonical bundle directory"
                ));
            }
            crate::bounded_file::read_bounded_file(
                &source,
                limit,
                &format!("Submission transport artifact {index}"),
            )
            .map_err(|error| public_artifact_read_error(error, limit, index))?
        } else {
            crate::bounded_file::read_bounded_frontier_file(
                frontier,
                relative,
                limit,
                &format!("Submission artifact {index}"),
            )
            .map_err(|error| public_artifact_read_error(error, limit, index))?
        };
        account_public_artifact_bytes(&mut total, bytes.len() as u64, index)?;
        let observed = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
        if observed != artifact.digest {
            return Err(format!(
                "Submission artifact {index} digest mismatch: declared {}, observed {observed}",
                artifact.digest
            ));
        }
        if !canonical_target.exists() {
            blobs.entry(canonical_path).or_insert(bytes);
        }
        read_set.push(InputBinding {
            name: format!("submission_artifact[{index}]"),
            digest: ContentDigest::parse(observed).map_err(|error| error.to_string())?,
        });
    }
    let writes = blobs
        .into_iter()
        .map(|(path, bytes)| {
            Ok(PlannedWrite::write(
                RepoPath::parse(path)?,
                WriteClass::CanonicalEvidence,
                bytes,
            ))
        })
        .collect::<Result<Vec<_>, crate::frontier_txn::FrontierTxnError>>()
        .map_err(|error| error.to_string())?;
    Ok(PreparedSubmissionArtifacts { writes, read_set })
}

pub(crate) fn submission_publication_inputs(
    frontier: &Path,
    submission: &SubmissionV1,
) -> Result<Vec<PathBuf>, String> {
    let canonical_frontier = frontier
        .canonicalize()
        .map_err(|error| format!("canonicalize frontier: {error}"))?;
    let mut inputs = submission
        .artifacts
        .iter()
        .map(|artifact| PathBuf::from(&artifact.path))
        .filter(|relative| {
            !relative.is_absolute()
                && relative
                    .components()
                    .all(|component| matches!(component, std::path::Component::Normal(_)))
        })
        .filter(|relative| {
            let lexical = canonical_frontier.join(relative);
            std::fs::symlink_metadata(&lexical)
                .ok()
                .is_some_and(|metadata| metadata.file_type().is_file())
                && lexical
                    .canonicalize()
                    .ok()
                    .is_some_and(|canonical| canonical == lexical)
        })
        .collect::<Vec<_>>();
    inputs.sort();
    inputs.dedup();
    Ok(inputs)
}

pub(crate) fn submit(
    frontier: &Path,
    submission: &SubmissionV1,
    executor: &str,
    bundle_root: Option<&Path>,
) -> Result<SubmitOutcome, String> {
    crate::current_submission::submit(frontier, submission, executor, bundle_root)
}

pub(crate) fn import_verification(
    frontier: &Path,
    record: &vela_protocol::verification_record::VerificationRecordV1,
    executor: &str,
) -> Result<VerificationImportOutcome, String> {
    crate::current_verification::import(frontier, record, executor)
}

pub(crate) fn frontier_transaction_journal_dir(frontier: &Path) -> Result<PathBuf, String> {
    let root = frontier
        .canonicalize()
        .map_err(|error| format!("resolve frontier transaction root: {error}"))?;
    let vela = root.join(".vela");
    let metadata = std::fs::symlink_metadata(&vela).map_err(|error| {
        format!(
            "inspect frontier private directory {}: {error}",
            vela.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "frontier private directory must be a real directory: {}",
            vela.display()
        ));
    }
    let journal = vela.join("operation-journals");
    match std::fs::symlink_metadata(&journal) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(format!(
            "frontier transaction journal must be a real directory: {}",
            journal.display()
        )),
        Ok(_) => Ok(journal),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(journal),
        Err(error) => Err(format!(
            "inspect frontier transaction journal {}: {error}",
            journal.display()
        )),
    }
}

fn account_public_artifact_bytes(
    total: &mut u64,
    artifact_bytes: u64,
    index: usize,
) -> Result<(), String> {
    let next = total.checked_add(artifact_bytes).ok_or_else(|| {
        format!("public artifact byte count overflowed while reading artifact {index}")
    })?;
    if next > crate::bounded_file::PUBLIC_ARTIFACT_TOTAL_MAX_BYTES {
        return Err(format!(
            "public artifacts exceed the {}-byte total limit at artifact {index}",
            crate::bounded_file::PUBLIC_ARTIFACT_TOTAL_MAX_BYTES
        ));
    }
    *total = next;
    Ok(())
}

fn public_artifact_read_limit(total: u64, index: usize) -> Result<u64, String> {
    let remaining = crate::bounded_file::PUBLIC_ARTIFACT_TOTAL_MAX_BYTES
        .checked_sub(total)
        .ok_or_else(|| {
            format!(
                "public artifacts already exceed the {}-byte total limit before artifact {index}",
                crate::bounded_file::PUBLIC_ARTIFACT_TOTAL_MAX_BYTES
            )
        })?;
    Ok(remaining.min(crate::bounded_file::PUBLIC_ARTIFACT_MAX_BYTES))
}

fn public_artifact_read_error(
    error: crate::bounded_file::BoundedFileError,
    read_limit: u64,
    index: usize,
) -> String {
    if error.code == "oversized" && read_limit < crate::bounded_file::PUBLIC_ARTIFACT_MAX_BYTES {
        format!(
            "public artifacts exceed the {}-byte total limit at artifact {index}",
            crate::bounded_file::PUBLIC_ARTIFACT_TOTAL_MAX_BYTES
        )
    } else {
        error.to_string()
    }
}

pub(crate) fn publication_delta(
    frontier: &Path,
    root: &str,
    writes: Vec<crate::frontier_txn::ResolvedWrite>,
) -> Result<Option<crate::config::git_publish::PublicationDelta>, String> {
    use crate::config::git_publish::{PublicationDelta, PublicationDeltaEntry};
    use crate::frontier_txn::{FileMode, FileState};
    if writes.is_empty() {
        return Ok(None);
    }
    let mut entries = writes
        .into_iter()
        .map(|write| {
            let path = crate::config::git_publish::publication_repo_relative_path(
                frontier,
                write.staged.path.as_str(),
            )?;
            let preimage_sha256 = match &write.staged.preimage {
                FileState::Absent => None,
                FileState::File { digest, .. } => Some(digest.as_str().to_string()),
            };
            let executable = matches!(
                write.staged.postimage,
                FileState::File {
                    mode: FileMode::Executable,
                    ..
                }
            );
            Ok(PublicationDeltaEntry {
                path,
                preimage_sha256,
                postimage: write.postimage_bytes,
                executable,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(Some(PublicationDelta {
        root: root.to_string(),
        entries,
    }))
}

#[cfg(test)]
mod tests {
    use super::submission_requested_change;

    fn root(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    #[test]
    fn submission_requested_change_defaults_to_add_claim() {
        let change = submission_requested_change(None, None, None).expect("add claim");

        assert_eq!(change.kind, "add_claim");
        assert!(change.target.is_none());
    }

    #[test]
    fn submission_requested_change_binds_exact_correction_target() {
        let claim_id = "vcl_exact_correction".to_string();
        let claim_root = root('a');
        let change =
            submission_requested_change(Some(claim_id.clone()), None, Some(claim_root.clone()))
                .expect("correction");
        let target = change.target.expect("correction target");

        assert_eq!(change.kind, "correct_claim");
        assert_eq!(target.claim_id, claim_id);
        assert_eq!(target.claim_root, claim_root);
    }

    #[test]
    fn submission_requested_change_binds_exact_supersession_target() {
        let claim_id = "vcl_exact_supersession".to_string();
        let claim_root = root('b');
        let change =
            submission_requested_change(None, Some(claim_id.clone()), Some(claim_root.clone()))
                .expect("supersession");
        let target = change.target.expect("supersession target");

        assert_eq!(change.kind, "supersede_claim");
        assert_eq!(target.claim_id, claim_id);
        assert_eq!(target.claim_root, claim_root);
    }

    #[test]
    fn submission_requested_change_rejects_ambiguous_or_incomplete_targets() {
        assert_eq!(
            submission_requested_change(
                Some("vcl_one".to_string()),
                Some("vcl_two".to_string()),
                Some(root('c')),
            )
            .expect_err("mutually exclusive"),
            "--corrects and --supersedes are mutually exclusive"
        );
        assert_eq!(
            submission_requested_change(Some("vcl_one".to_string()), None, None)
                .expect_err("missing root"),
            "--corrects and --supersedes require --target-root"
        );
        assert_eq!(
            submission_requested_change(None, None, Some(root('d'))).expect_err("orphan root"),
            "--target-root requires --corrects or --supersedes"
        );
    }
}
