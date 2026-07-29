//! Read-only preparation for the one-time pre-release repository compaction.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vela_protocol::claim_record::{ClaimEvidenceRef, ClaimRecordV1, LEGACY_FINDING_EXTENSION};
use vela_protocol::current_repository::{
    CURRENT_ARTIFACT_RECORD_SCHEMA_V1, CURRENT_REPOSITORY_SCHEMA_V3, ClaimStandingRefV1,
    CurrentArtifactRecordV1, CurrentRepositoryV3, RepositoryObjectRefV1,
};
use vela_protocol::current_state_equivalence::{
    ArtifactCompactionMapV1, ClaimCompactionMapV1, CompactedArtifactForm, CurrentStateEquivalenceV1,
};
use vela_protocol::repository_epoch::RepositoryBoundaryV1;
use vela_protocol::repository_origin::{RepositoryOriginPredecessorV1, RepositoryOriginV1};

struct ClaimCompactionAudit {
    report: CurrentStateEquivalenceV1,
    claim_map_root: String,
    candidate_claims: Vec<ClaimRecordV1>,
    candidate_objects: Vec<RepositoryObjectRefV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateObjectManifestV1 {
    schema: String,
    frontier_id: String,
    objects: Vec<RepositoryObjectRefV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompactionCandidatePlanV1 {
    schema: String,
    frontier_id: String,
    source_remote: String,
    source_commit: String,
    source_tree: String,
    predecessor_repository_root: String,
    predecessor_boundary_id: String,
    predecessor_boundary_root: String,
    predecessor_authority_head_root: String,
    predecessor_event_log_root: String,
    predecessor_actor_registry_root: String,
    predecessor_tag: String,
    predecessor_archive_root: String,
    predecessor_object_manifest_root: String,
    artifact_map_root: String,
    claim_map_root: String,
    equivalence_report_root: String,
    candidate_object_manifest_root: String,
    candidate_object_set_root: String,
    candidate_origin_id: String,
    candidate_origin_root: String,
    candidate_repository_root: String,
    touched_paths: Vec<String>,
    reason: String,
}

pub(crate) fn cmd_compaction_check(
    frontier: &Path,
    check: bool,
    output: Option<&Path>,
    json_out: bool,
) {
    crate::ui::set_mode("repository compact", json_out);
    if !check {
        crate::cli::fail_return::<()>(
            "repository compact is currently preview-only and requires --check",
        );
    }
    let frontier = frontier.canonicalize().unwrap_or_else(|error| {
        crate::cli::fail_return(&format!(
            "resolve current Frontier {}: {error}",
            frontier.display()
        ))
    });
    let mappings = audit_artifact_compaction(&frontier)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let local = mappings
        .iter()
        .filter(|mapping| mapping.form == CompactedArtifactForm::LocalBlob)
        .count();
    let remote = mappings.len() - local;
    let map_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&mappings)
            .unwrap_or_else(|error| crate::cli::fail_return(&error))
    );
    let claim_audit = audit_claim_compaction(&frontier, &mappings)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let equivalence_report_root = claim_audit
        .report
        .canonical_root()
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let materialized = output.map(|output| {
        materialize_candidate(
            &frontier,
            output,
            &mappings,
            &claim_audit,
            &map_root,
            &equivalence_report_root,
        )
        .unwrap_or_else(|error| crate::cli::fail_return(&error))
    });
    let payload = serde_json::json!({
        "schema": "vela.repository-compaction-check.v1",
        "ok": true,
        "command": "repository compact",
        "writes_now": false,
        "frontier": frontier,
        "artifact_map_root": map_root,
        "counts": {
            "imported_artifact_wrappers": mappings.len(),
            "local_blobs": local,
            "external_references": remote,
        },
        "claim_map_root": claim_audit.claim_map_root,
        "equivalence_report_root": equivalence_report_root,
        "accepted_claims": claim_audit.report.accepted_count_before,
        "relations": claim_audit.report.relation_count_before,
        "archived_live_objects": claim_audit.report.archived_live_object_roots.len(),
        "candidate_object_set_root": claim_audit.report.candidate_object_set_root,
        "equivalent": claim_audit.report.equivalent,
        "candidate": materialized,
        "next_action": if output.is_some() {
            "verify the isolated candidate and bind its repository-origin plan before any authority action"
        } else {
            "rerun with --output outside the Frontier to materialize the exact isolated candidate"
        },
    });
    if json_out {
        crate::cli::print_json(&payload);
    } else {
        println!("pre-release Artifact compaction is structurally ready");
        println!("  wrappers: {}", mappings.len());
        println!("  local blobs: {local}");
        println!("  external references: {remote}");
        println!("  map root: {map_root}");
        println!(
            "  accepted Claims: {}",
            claim_audit.report.accepted_count_before
        );
        println!("  equivalence report: {}", equivalence_report_root);
        if let Some(materialized) = materialized {
            println!("  candidate: {}", materialized["output"]);
            println!("  candidate plan: {}", materialized["plan_root"]);
        }
        println!("  source writes: no");
    }
}

pub(crate) fn audit_artifact_compaction(
    frontier: &Path,
) -> Result<Vec<ArtifactCompactionMapV1>, String> {
    let repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    let mut mappings = Vec::new();
    for reference in &repository.artifacts {
        if reference.schema != CURRENT_ARTIFACT_RECORD_SCHEMA_V1 {
            continue;
        }
        let bytes = fs::read(frontier.join(&reference.path))
            .map_err(|error| format!("read {}: {error}", reference.path))?;
        let record = CurrentArtifactRecordV1::parse(&bytes)?;
        mappings.push(map_imported_artifact(frontier, reference, &record, &bytes)?);
    }
    mappings.sort_by(|left, right| {
        left.predecessor_artifact_id
            .cmp(&right.predecessor_artifact_id)
    });
    Ok(mappings)
}

fn audit_claim_compaction(
    frontier: &Path,
    artifact_map: &[ArtifactCompactionMapV1],
) -> Result<ClaimCompactionAudit, String> {
    let repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    let artifact_by_predecessor = artifact_map
        .iter()
        .map(|mapping| (mapping.predecessor_artifact_id.as_str(), mapping))
        .collect::<BTreeMap<_, _>>();
    let mut original_claims = Vec::new();
    let mut first_pass = Vec::new();
    let mut claim_id_map = BTreeMap::new();
    let retained_claim_ids = retained_claim_ids(frontier)?;

    for reference in &repository.accepted_claims {
        let bytes = fs::read(frontier.join(&reference.path))
            .map_err(|error| format!("read {}: {error}", reference.path))?;
        let claim = ClaimRecordV1::parse(&bytes)?;
        if claim.claim_id != reference.claim_id || claim.canonical_root()? != reference.claim_root {
            return Err(format!(
                "accepted Claim {} differs from its repository reference",
                reference.claim_id
            ));
        }
        let evidence =
            normalize_claim_evidence(&claim.evidence, &artifact_by_predecessor, &repository)?;
        let mut extensions = claim.extensions.clone();
        extensions.remove(LEGACY_FINDING_EXTENSION);
        let candidate = ClaimRecordV1::build(
            claim.revision,
            claim.assertion.clone(),
            claim.conditions.clone(),
            evidence,
            claim.provenance.clone(),
            claim.relations.clone(),
            claim.created_at.clone(),
            None,
            extensions,
        )?;
        if claim_id_map
            .insert(claim.claim_id.clone(), candidate.claim_id.clone())
            .is_some()
        {
            return Err(format!(
                "accepted Claim {} appears more than once",
                claim.claim_id
            ));
        }
        original_claims.push((reference.clone(), claim));
        first_pass.push(candidate);
    }

    let mut candidate_ids = BTreeSet::new();
    let mut candidate_claims = Vec::new();
    let mut claim_map = Vec::new();
    let mut relation_count = 0_u64;
    for ((reference, original), first) in original_claims.iter().zip(first_pass) {
        let relations = original
            .relations
            .iter()
            .map(|relation| {
                let target_claim_id = claim_id_map
                    .get(&relation.target_claim_id)
                    .cloned()
                    .or_else(|| {
                        retained_claim_ids
                            .contains(&relation.target_claim_id)
                            .then(|| relation.target_claim_id.clone())
                    })
                    .ok_or_else(|| {
                        format!(
                            "accepted Claim {} relates to Claim {} absent from the retained predecessor",
                            original.claim_id, relation.target_claim_id
                        )
                    })?;
                Ok(vela_protocol::claim_record::ClaimRelation {
                    kind: relation.kind.clone(),
                    target_claim_id,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        relation_count += relations.len() as u64;
        let candidate = ClaimRecordV1::build(
            first.revision,
            first.assertion.clone(),
            first.conditions.clone(),
            first.evidence.clone(),
            first.provenance.clone(),
            relations.clone(),
            first.created_at.clone(),
            None,
            first.extensions.clone(),
        )?;
        if candidate.claim_id != first.claim_id {
            return Err(format!(
                "relation remapping changed Claim identity for {}",
                original.claim_id
            ));
        }
        if !candidate_ids.insert(candidate.claim_id.clone()) {
            return Err(format!(
                "Claim compaction collapses {} into duplicate {}",
                original.claim_id, candidate.claim_id
            ));
        }
        let predecessor_projection_root = scientific_projection_root(
            original,
            &first.evidence,
            &relations,
            &artifact_by_predecessor,
        )?;
        let candidate_projection_root = scientific_projection_root(
            &candidate,
            &candidate.evidence,
            &candidate.relations,
            &artifact_by_predecessor,
        )?;
        claim_map.push(ClaimCompactionMapV1 {
            predecessor_claim_id: original.claim_id.clone(),
            predecessor_claim_root: reference.claim_root.clone(),
            candidate_claim_id: candidate.claim_id.clone(),
            candidate_claim_root: candidate.canonical_root()?,
            standing: "accepted".into(),
            predecessor_projection_root,
            candidate_projection_root,
        });
        candidate_claims.push(candidate);
    }
    claim_map.sort_by(|left, right| left.predecessor_claim_id.cmp(&right.predecessor_claim_id));

    let mut candidate_objects = artifact_map
        .iter()
        .map(|mapping| RepositoryObjectRefV1 {
            schema: "content-addressed-artifact".into(),
            id: mapping.candidate_artifact_id.clone(),
            root: mapping.candidate_artifact_root.clone(),
            path: format!("records/artifacts/sha256/{}", mapping.candidate_artifact_id),
        })
        .collect::<Vec<_>>();
    let mapped_artifact_ids = artifact_map
        .iter()
        .map(|mapping| mapping.candidate_artifact_id.as_str())
        .collect::<BTreeSet<_>>();
    for claim in &candidate_claims {
        for evidence in &claim.evidence {
            let Some(artifact_id) = evidence.artifact_id.as_deref() else {
                continue;
            };
            if mapped_artifact_ids.contains(artifact_id) {
                continue;
            }
            let reference = repository
                .artifacts
                .iter()
                .find(|reference| reference.id == artifact_id)
                .ok_or_else(|| {
                    format!(
                        "compacted Claim {} references absent current Artifact {}",
                        claim.claim_id, artifact_id
                    )
                })?;
            if reference.root != evidence.artifact_root
                || evidence.artifact_path.as_deref() != Some(reference.path.as_str())
            {
                return Err(format!(
                    "compacted Claim {} disagrees with current Artifact {}",
                    claim.claim_id, artifact_id
                ));
            }
            candidate_objects.push(reference.clone());
        }
    }
    for claim in &candidate_claims {
        let root = claim.canonical_root()?;
        candidate_objects.push(RepositoryObjectRefV1 {
            schema: claim.schema.clone(),
            id: claim.claim_id.clone(),
            root: root.clone(),
            path: format!(
                "records/claims/sha256/{}.json",
                root.trim_start_matches("sha256:")
            ),
        });
    }
    candidate_objects.sort_by(|left, right| {
        (&left.schema, &left.id, &left.root, &left.path).cmp(&(
            &right.schema,
            &right.id,
            &right.root,
            &right.path,
        ))
    });
    candidate_objects.dedup();
    let candidate_object_set_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&candidate_objects)?
    );
    let mut archived_live_object_roots = repository
        .proposals
        .iter()
        .chain(&repository.submissions)
        .chain(&repository.registrations)
        .chain(&repository.verifications)
        .map(|reference| reference.root.clone())
        .collect::<Vec<_>>();
    archived_live_object_roots.sort();
    archived_live_object_roots.dedup();
    let predecessor_repository_root = repository.canonical_root()?;
    let accepted_count = claim_map.len() as u64;
    let report = CurrentStateEquivalenceV1::build(
        repository.frontier_id,
        predecessor_repository_root,
        candidate_object_set_root,
        artifact_map.to_vec(),
        claim_map.clone(),
        archived_live_object_roots,
        accepted_count,
        accepted_count,
        relation_count,
        relation_count,
    )?;
    let claim_map_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&claim_map)?
    );
    Ok(ClaimCompactionAudit {
        report,
        claim_map_root,
        candidate_claims,
        candidate_objects,
    })
}

fn materialize_candidate(
    frontier: &Path,
    output: &Path,
    artifact_map: &[ArtifactCompactionMapV1],
    claim_audit: &ClaimCompactionAudit,
    artifact_map_root: &str,
    equivalence_report_root: &str,
) -> Result<serde_json::Value, String> {
    let output = absolute_output_path(output)?;
    if output.starts_with(frontier) {
        return Err("compaction candidate output must be outside the source Frontier".into());
    }
    if output.exists() {
        return Err(format!(
            "compaction candidate output already exists at {}",
            output.display()
        ));
    }
    let parent = output
        .parent()
        .ok_or_else(|| "compaction candidate output has no parent".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create candidate parent {}: {error}", parent.display()))?;
    let file_name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "compaction candidate output has no UTF-8 file name".to_string())?;
    let temporary = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_dir_all(&temporary).map_err(|error| {
            format!(
                "remove stale candidate temporary directory {}: {error}",
                temporary.display()
            )
        })?;
    }
    fs::create_dir(&temporary).map_err(|error| {
        format!(
            "create candidate temporary directory {}: {error}",
            temporary.display()
        )
    })?;
    let result = materialize_candidate_inner(
        frontier,
        &temporary,
        artifact_map,
        claim_audit,
        artifact_map_root,
        equivalence_report_root,
    );
    let value = match result {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(&temporary);
            return Err(error);
        }
    };
    fs::rename(&temporary, &output).map_err(|error| {
        let _ = fs::remove_dir_all(&temporary);
        format!("publish candidate directory {}: {error}", output.display())
    })?;
    Ok(serde_json::json!({
        "output": output,
        "plan_root": value["plan_root"],
        "object_manifest_root": value["object_manifest_root"],
        "file_count": value["file_count"],
        "verified": value["verified"],
    }))
}

fn materialize_candidate_inner(
    frontier: &Path,
    output: &Path,
    artifact_map: &[ArtifactCompactionMapV1],
    claim_audit: &ClaimCompactionAudit,
    artifact_map_root: &str,
    equivalence_report_root: &str,
) -> Result<serde_json::Value, String> {
    let repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    let epoch_bytes = fs::read(frontier.join(".vela/epoch.json"))
        .map_err(|error| format!("read current repository boundary: {error}"))?;
    let epoch = RepositoryBoundaryV1::parse(&epoch_bytes)?;
    let authority = crate::cli::load_current_repository_authority(frontier, &repository, &epoch)?;
    let authority_head = authority
        .verification
        .final_authority_record_root
        .clone()
        .ok_or_else(|| "current repository authority has no final record root".to_string())?;

    let mapping_by_candidate = artifact_map
        .iter()
        .map(|mapping| (mapping.candidate_artifact_id.as_str(), mapping))
        .collect::<BTreeMap<_, _>>();
    let current_by_id = repository
        .artifacts
        .iter()
        .map(|reference| (reference.id.as_str(), reference))
        .collect::<BTreeMap<_, _>>();

    for reference in claim_audit
        .candidate_objects
        .iter()
        .filter(|reference| reference.schema == "content-addressed-artifact")
    {
        let mapping = mapping_by_candidate
            .get(reference.id.as_str())
            .ok_or_else(|| format!("candidate Artifact {} has no compaction map", reference.id))?;
        let predecessor = current_by_id
            .get(mapping.predecessor_artifact_id.as_str())
            .ok_or_else(|| {
                format!(
                    "compaction map names absent predecessor Artifact {}",
                    mapping.predecessor_artifact_id
                )
            })?;
        let predecessor_bytes = fs::read(frontier.join(&predecessor.path))
            .map_err(|error| format!("read {}: {error}", predecessor.path))?;
        let record = CurrentArtifactRecordV1::parse(&predecessor_bytes)?;
        let bytes = match mapping.form {
            CompactedArtifactForm::LocalBlob => {
                let locator = record
                    .artifact
                    .get("locator")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        format!(
                            "predecessor Artifact {} has no local locator",
                            predecessor.id
                        )
                    })?;
                fs::read(frontier.join(locator)).map_err(|error| {
                    format!("read retained Artifact bytes at {locator}: {error}")
                })?
            }
            CompactedArtifactForm::ExternalReference => predecessor_bytes,
        };
        write_exact_candidate(output, &reference.path, &bytes, &reference.root)?;
    }

    for reference in claim_audit.candidate_objects.iter().filter(|reference| {
        reference.schema != "content-addressed-artifact"
            && reference.schema != vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA
    }) {
        let bytes = fs::read(frontier.join(&reference.path))
            .map_err(|error| format!("read retained current object {}: {error}", reference.path))?;
        write_exact_candidate(output, &reference.path, &bytes, &reference.root)?;
    }
    let claims_by_id = claim_audit
        .candidate_claims
        .iter()
        .map(|claim| (claim.claim_id.as_str(), claim))
        .collect::<BTreeMap<_, _>>();
    for reference in claim_audit
        .candidate_objects
        .iter()
        .filter(|reference| reference.schema == vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA)
    {
        let claim = claims_by_id
            .get(reference.id.as_str())
            .ok_or_else(|| format!("candidate Claim {} has no rebuilt bytes", reference.id))?;
        write_exact_candidate(
            output,
            &reference.path,
            &claim.canonical_bytes()?,
            &reference.root,
        )?;
    }

    let object_manifest = CandidateObjectManifestV1 {
        schema: "vela.repository-compaction-object-manifest.v1".into(),
        frontier_id: repository.frontier_id.clone(),
        objects: claim_audit.candidate_objects.clone(),
    };
    let object_manifest_bytes = vela_protocol::canonical::to_canonical_bytes(&object_manifest)?;
    let object_manifest_root = root_bytes(&object_manifest_bytes);
    write_relative(output, "object-manifest.json", &object_manifest_bytes)?;
    write_relative(
        output,
        "equivalence.json",
        &claim_audit.report.canonical_bytes()?,
    )?;

    let source_commit = git_output(frontier, &["rev-parse", "HEAD"])?;
    let source_tree = git_output(frontier, &["rev-parse", "HEAD^{tree}"])?;
    let source_remote = canonical_remote(&git_output(frontier, &["remote", "get-url", "origin"])?);
    let tag = format!("pre-compaction/{}", &source_commit[..12]);
    let archive_path = output.join("predecessor.tar");
    git_archive(frontier, &archive_path)?;
    let predecessor_archive_root = root_bytes(
        &fs::read(&archive_path)
            .map_err(|error| format!("read predecessor Git bundle: {error}"))?,
    );
    let predecessor_object_manifest_root = predecessor_object_manifest_root(&repository)?;
    let predecessor_roots = epoch.predecessor_roots().ok_or_else(|| {
        "pre-release compaction requires a predecessor-bound current repository".to_string()
    })?;
    let predecessor_event_log_root = authority.verification.final_event_log_root.clone();
    let predecessor_actor_registry_root = predecessor_roots.actor_registry.clone();
    let origin = RepositoryOriginV1::compaction(
        repository.frontier_id.clone(),
        2,
        repository.profile_root.clone(),
        claim_audit.report.candidate_object_set_root.clone(),
        RepositoryOriginPredecessorV1 {
            remote: source_remote.clone(),
            tag: tag.clone(),
            commit: source_commit.clone(),
            tree: source_tree.clone(),
            repository_root: repository.canonical_root()?,
            authority_head_root: authority_head.clone(),
            archived_event_log_root: predecessor_event_log_root.clone(),
            archived_actor_registry_root: predecessor_actor_registry_root.clone(),
            archive_sha256: predecessor_archive_root.clone(),
            object_manifest_root: predecessor_object_manifest_root.clone(),
            equivalence_report_root: equivalence_report_root.into(),
        },
        "Compact the unreleased repository into the single current origin and content-addressed evidence contract.".into(),
    )?;
    let origin_root = origin.canonical_root()?;
    write_relative(output, ".vela/origin.json", &origin.canonical_bytes()?)?;

    let mut accepted_claims = claim_audit
        .candidate_claims
        .iter()
        .map(|claim| {
            let root = claim.canonical_root()?;
            Ok(ClaimStandingRefV1 {
                claim_id: claim.claim_id.clone(),
                claim_root: root.clone(),
                standing: "accepted".into(),
                path: format!(
                    "records/claims/sha256/{}.json",
                    root.trim_start_matches("sha256:")
                ),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    accepted_claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let mut artifacts = claim_audit
        .candidate_objects
        .iter()
        .filter(|reference| reference.schema != vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA)
        .cloned()
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| left.id.cmp(&right.id));
    let candidate_repository = CurrentRepositoryV3 {
        schema: CURRENT_REPOSITORY_SCHEMA_V3.into(),
        frontier_id: repository.frontier_id.clone(),
        profile_root: repository.profile_root.clone(),
        origin_id: origin.origin_id.clone(),
        origin_root: origin_root.clone(),
        accepted_claims,
        pending_claims: Vec::new(),
        proposals: Vec::new(),
        submissions: Vec::new(),
        registrations: Vec::new(),
        verifications: Vec::new(),
        artifacts,
        authority_keyset_root: repository.authority_keyset_root.clone(),
        authority_policy_root: repository.authority_policy_root.clone(),
    };
    let candidate_repository_root = candidate_repository.canonical_root()?;
    write_relative(
        output,
        ".vela/repository.json",
        &candidate_repository.canonical_bytes()?,
    )?;
    let mut touched_paths = claim_audit
        .candidate_objects
        .iter()
        .map(|reference| reference.path.clone())
        .collect::<Vec<_>>();
    touched_paths.extend([
        ".vela/origin.json".into(),
        ".vela/repository.json".into(),
        "equivalence.json".into(),
        "object-manifest.json".into(),
    ]);
    touched_paths.sort();
    touched_paths.dedup();
    let predecessor_repository_root = repository.canonical_root()?;
    let plan = CompactionCandidatePlanV1 {
        schema: "vela.repository-compaction-candidate.v1".into(),
        frontier_id: repository.frontier_id,
        source_remote,
        source_commit,
        source_tree,
        predecessor_repository_root,
        predecessor_boundary_id: epoch.epoch_id().to_string(),
        predecessor_boundary_root: epoch.canonical_root()?,
        predecessor_authority_head_root: authority_head,
        predecessor_event_log_root,
        predecessor_actor_registry_root,
        predecessor_tag: tag,
        predecessor_archive_root,
        predecessor_object_manifest_root,
        artifact_map_root: artifact_map_root.into(),
        claim_map_root: claim_audit.claim_map_root.clone(),
        equivalence_report_root: equivalence_report_root.into(),
        candidate_object_manifest_root: object_manifest_root.clone(),
        candidate_object_set_root: claim_audit.report.candidate_object_set_root.clone(),
        candidate_origin_id: origin.origin_id,
        candidate_origin_root: origin_root,
        candidate_repository_root,
        touched_paths,
        reason: "Adopt one current pre-release repository origin and content-addressed evidence contract.".into(),
    };
    let plan_bytes = vela_protocol::canonical::to_canonical_bytes(&plan)?;
    let plan_root = root_bytes(&plan_bytes);
    write_relative(output, "plan.json", &plan_bytes)?;
    let file_count = verify_materialized_candidate(output)?;
    Ok(serde_json::json!({
        "plan_root": plan_root,
        "object_manifest_root": object_manifest_root,
        "file_count": file_count,
        "verified": true,
    }))
}

fn verify_materialized_candidate(root: &Path) -> Result<usize, String> {
    let plan_bytes = fs::read(root.join("plan.json"))
        .map_err(|error| format!("read candidate plan: {error}"))?;
    let plan: CompactionCandidatePlanV1 = serde_json::from_slice(&plan_bytes)
        .map_err(|error| format!("parse candidate plan: {error}"))?;
    if vela_protocol::canonical::to_canonical_bytes(&plan)? != plan_bytes {
        return Err("candidate plan is not canonical JSON".into());
    }
    let manifest_bytes = fs::read(root.join("object-manifest.json"))
        .map_err(|error| format!("read candidate object manifest: {error}"))?;
    let manifest: CandidateObjectManifestV1 = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse candidate object manifest: {error}"))?;
    if vela_protocol::canonical::to_canonical_bytes(&manifest)? != manifest_bytes {
        return Err("candidate object manifest is not canonical JSON".into());
    }
    if manifest.frontier_id != plan.frontier_id
        || root_bytes(&manifest_bytes) != plan.candidate_object_manifest_root
    {
        return Err("candidate object manifest disagrees with its plan".into());
    }
    let object_set_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&manifest.objects)?
    );
    if object_set_root != plan.candidate_object_set_root {
        return Err("candidate object set disagrees with its plan".into());
    }

    let equivalence_bytes = fs::read(root.join("equivalence.json"))
        .map_err(|error| format!("read candidate equivalence report: {error}"))?;
    let report = CurrentStateEquivalenceV1::parse(&equivalence_bytes)?;
    if report.frontier_id != plan.frontier_id
        || report.predecessor_repository_root != plan.predecessor_repository_root
        || report.candidate_object_set_root != plan.candidate_object_set_root
        || report.canonical_root()? != plan.equivalence_report_root
    {
        return Err("candidate equivalence report disagrees with its plan".into());
    }
    let archive_bytes = fs::read(root.join("predecessor.tar"))
        .map_err(|error| format!("read candidate predecessor archive: {error}"))?;
    if root_bytes(&archive_bytes) != plan.predecessor_archive_root {
        return Err("candidate predecessor archive disagrees with its plan".into());
    }
    let origin_bytes = fs::read(root.join(".vela/origin.json"))
        .map_err(|error| format!("read candidate origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    if origin.frontier_id != plan.frontier_id
        || origin.origin_id != plan.candidate_origin_id
        || origin.canonical_root()? != plan.candidate_origin_root
        || origin.initial_object_set_root != plan.candidate_object_set_root
    {
        return Err("candidate repository origin disagrees with its plan".into());
    }
    let predecessor = origin
        .predecessor
        .as_ref()
        .ok_or_else(|| "candidate compaction origin has no predecessor".to_string())?;
    if predecessor.repository_root != plan.predecessor_repository_root
        || predecessor.authority_head_root != plan.predecessor_authority_head_root
        || predecessor.archived_event_log_root != plan.predecessor_event_log_root
        || predecessor.archived_actor_registry_root != plan.predecessor_actor_registry_root
        || predecessor.tag != plan.predecessor_tag
        || predecessor.commit != plan.source_commit
        || predecessor.tree != plan.source_tree
        || predecessor.remote != plan.source_remote
        || predecessor.archive_sha256 != plan.predecessor_archive_root
        || predecessor.object_manifest_root != plan.predecessor_object_manifest_root
        || predecessor.equivalence_report_root != plan.equivalence_report_root
    {
        return Err("candidate repository origin predecessor disagrees with its plan".into());
    }
    let repository_bytes = fs::read(root.join(".vela/repository.json"))
        .map_err(|error| format!("read candidate repository: {error}"))?;
    let repository = CurrentRepositoryV3::parse(&repository_bytes)?;
    if repository.frontier_id != plan.frontier_id
        || repository.profile_root != origin.profile_root
        || repository.origin_id != origin.origin_id
        || repository.origin_root != plan.candidate_origin_root
        || repository.canonical_root()? != plan.candidate_repository_root
    {
        return Err("candidate repository manifest disagrees with its plan".into());
    }
    let mut manifest_claims = manifest
        .objects
        .iter()
        .filter(|reference| reference.schema == vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA)
        .map(|reference| ClaimStandingRefV1 {
            claim_id: reference.id.clone(),
            claim_root: reference.root.clone(),
            standing: "accepted".into(),
            path: reference.path.clone(),
        })
        .collect::<Vec<_>>();
    manifest_claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let mut manifest_artifacts = manifest
        .objects
        .iter()
        .filter(|reference| reference.schema != vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA)
        .cloned()
        .collect::<Vec<_>>();
    manifest_artifacts.sort_by(|left, right| left.id.cmp(&right.id));
    if repository.accepted_claims != manifest_claims
        || repository.artifacts != manifest_artifacts
        || !repository.pending_claims.is_empty()
        || !repository.proposals.is_empty()
        || !repository.submissions.is_empty()
        || !repository.registrations.is_empty()
        || !repository.verifications.is_empty()
    {
        return Err(
            "candidate repository object sets disagree with the exact candidate manifest".into(),
        );
    }

    let objects_by_id = manifest
        .objects
        .iter()
        .map(|reference| (reference.id.as_str(), reference))
        .collect::<BTreeMap<_, _>>();
    for mapping in &report.artifact_map {
        let reference = objects_by_id
            .get(mapping.candidate_artifact_id.as_str())
            .ok_or_else(|| {
                format!(
                    "candidate manifest lacks compacted Artifact {}",
                    mapping.candidate_artifact_id
                )
            })?;
        if reference.root != mapping.candidate_artifact_root
            || reference.schema != "content-addressed-artifact"
        {
            return Err(format!(
                "candidate Artifact {} disagrees with the equivalence map",
                mapping.candidate_artifact_id
            ));
        }
    }
    for mapping in &report.claim_map {
        let reference = objects_by_id
            .get(mapping.candidate_claim_id.as_str())
            .ok_or_else(|| {
                format!(
                    "candidate manifest lacks compacted Claim {}",
                    mapping.candidate_claim_id
                )
            })?;
        if reference.root != mapping.candidate_claim_root
            || reference.schema != vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA
        {
            return Err(format!(
                "candidate Claim {} disagrees with the equivalence map",
                mapping.candidate_claim_id
            ));
        }
    }

    for reference in &manifest.objects {
        let bytes = fs::read(root.join(&reference.path))
            .map_err(|error| format!("read candidate object {}: {error}", reference.path))?;
        if root_bytes(&bytes) != reference.root {
            return Err(format!(
                "candidate object {} does not match {}",
                reference.path, reference.root
            ));
        }
        if reference.schema == vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA {
            let claim = ClaimRecordV1::parse(&bytes)?;
            if claim.claim_id != reference.id
                || claim
                    .evidence
                    .iter()
                    .filter_map(|evidence| evidence.artifact_id.as_deref())
                    .any(|artifact_id| artifact_id.starts_with("va_"))
            {
                return Err(format!(
                    "candidate Claim {} retains a legacy Artifact identity",
                    reference.id
                ));
            }
        }
        if reference.schema == CURRENT_ARTIFACT_RECORD_SCHEMA_V1 || reference.id.starts_with("va_")
        {
            return Err(format!(
                "candidate manifest retains legacy Artifact wrapper {}",
                reference.id
            ));
        }
    }

    let files = walk_files(root)?;
    let expected = manifest
        .objects
        .iter()
        .map(|reference| root.join(&reference.path))
        .chain(
            ["plan.json", "object-manifest.json", "equivalence.json"]
                .into_iter()
                .map(|path| root.join(path)),
        )
        .chain(
            [
                "predecessor.tar",
                ".vela/origin.json",
                ".vela/repository.json",
            ]
            .into_iter()
            .map(|path| root.join(path)),
        )
        .collect::<BTreeSet<_>>();
    let observed = files.iter().cloned().collect::<BTreeSet<_>>();
    if observed != expected {
        return Err("candidate package contains missing or unexplained files".into());
    }
    Ok(files.len())
}

fn write_exact_candidate(
    output: &Path,
    relative: &str,
    bytes: &[u8],
    expected_root: &str,
) -> Result<(), String> {
    let observed = root_bytes(bytes);
    if observed != expected_root {
        return Err(format!(
            "candidate object {relative} has root {observed}, expected {expected_root}"
        ));
    }
    write_relative(output, relative, bytes)
}

fn write_relative(output: &Path, relative: &str, bytes: &[u8]) -> Result<(), String> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!("candidate path `{relative}` is not safe"));
    }
    let destination = output.join(path);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create candidate path {}: {error}", parent.display()))?;
    }
    fs::write(&destination, bytes)
        .map_err(|error| format!("write candidate object {}: {error}", destination.display()))
}

fn root_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn absolute_output_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|current| current.join(path))
        .map_err(|error| format!("resolve candidate output: {error}"))
}

fn git_output(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("decode git {} output: {error}", args.join(" ")))
        .map(|value| value.trim().to_string())
}

fn git_archive(root: &Path, destination: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["archive", "--format=tar", "--output"])
        .arg(destination)
        .arg("HEAD")
        .output()
        .map_err(|error| format!("create predecessor Git archive: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "create predecessor Git archive: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn predecessor_object_manifest_root(
    repository: &vela_protocol::current_repository::CurrentRepositoryV2,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct RootedObject<'a> {
        kind: &'a str,
        id: &'a str,
        root: &'a str,
        path: &'a str,
    }
    let mut objects = repository
        .accepted_claims
        .iter()
        .map(|reference| RootedObject {
            kind: "accepted_claim",
            id: &reference.claim_id,
            root: &reference.claim_root,
            path: &reference.path,
        })
        .chain(
            repository
                .pending_claims
                .iter()
                .map(|reference| RootedObject {
                    kind: "pending_claim",
                    id: &reference.claim_id,
                    root: &reference.claim_root,
                    path: &reference.path,
                }),
        )
        .chain(
            [
                ("proposal", &repository.proposals),
                ("submission", &repository.submissions),
                ("registration", &repository.registrations),
                ("verification", &repository.verifications),
                ("artifact", &repository.artifacts),
            ]
            .into_iter()
            .flat_map(|(kind, references)| {
                references.iter().map(move |reference| RootedObject {
                    kind,
                    id: &reference.id,
                    root: &reference.root,
                    path: &reference.path,
                })
            }),
        )
        .collect::<Vec<_>>();
    objects.sort_by(|left, right| {
        (left.kind, left.id, left.root, left.path)
            .cmp(&(right.kind, right.id, right.root, right.path))
    });
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&objects)?
    ))
}

fn canonical_remote(remote: &str) -> String {
    if let Some(path) = remote.strip_prefix("git@github.com:") {
        return format!("https://github.com/{path}");
    }
    remote.to_string()
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("read candidate directory {}: {error}", directory.display()))?
        {
            let entry = entry.map_err(|error| format!("read candidate entry: {error}"))?;
            let file_type = entry
                .file_type()
                .map_err(|error| format!("inspect candidate entry: {error}"))?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                files.push(entry.path());
            } else {
                return Err(format!(
                    "candidate contains unsupported path {}",
                    entry.path().display()
                ));
            }
        }
    }
    files.sort();
    Ok(files)
}

fn retained_claim_ids(frontier: &Path) -> Result<BTreeSet<String>, String> {
    let directory = frontier.join("records/claims/sha256");
    let mut ids = BTreeSet::new();
    for entry in fs::read_dir(&directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?
    {
        let path = entry
            .map_err(|error| format!("read Claim directory entry: {error}"))?
            .path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
        let claim = ClaimRecordV1::parse(&bytes)
            .map_err(|error| format!("parse {}: {error}", path.display()))?;
        if !ids.insert(claim.claim_id.clone()) {
            return Err(format!(
                "retained predecessor has duplicate Claim {}",
                claim.claim_id
            ));
        }
    }
    Ok(ids)
}

fn normalize_claim_evidence(
    evidence: &[ClaimEvidenceRef],
    artifact_by_predecessor: &BTreeMap<&str, &ArtifactCompactionMapV1>,
    repository: &vela_protocol::current_repository::CurrentRepositoryV2,
) -> Result<Vec<ClaimEvidenceRef>, String> {
    evidence
        .iter()
        .map(|item| {
            let Some(artifact_id) = item.artifact_id.as_deref() else {
                return Ok(item.clone());
            };
            if let Some(mapping) = artifact_by_predecessor.get(artifact_id) {
                if item.artifact_root != mapping.predecessor_record_root {
                    return Err(format!(
                        "Claim evidence for {} disagrees with its imported Artifact root",
                        artifact_id
                    ));
                }
                return Ok(ClaimEvidenceRef {
                    relation: item.relation.clone(),
                    artifact_id: Some(mapping.candidate_artifact_id.clone()),
                    artifact_root: mapping.candidate_artifact_root.clone(),
                    artifact_path: Some(format!(
                        "records/artifacts/sha256/{}",
                        mapping.candidate_artifact_id
                    )),
                });
            }
            let reference = repository
                .artifacts
                .iter()
                .find(|reference| reference.id == artifact_id)
                .ok_or_else(|| {
                    format!(
                        "Claim evidence names Artifact {} outside the repository",
                        artifact_id
                    )
                })?;
            if reference.root != item.artifact_root
                || item.artifact_path.as_deref() != Some(reference.path.as_str())
            {
                return Err(format!(
                    "Claim evidence for {} disagrees with its current Artifact reference",
                    artifact_id
                ));
            }
            Ok(item.clone())
        })
        .collect()
}

fn scientific_projection_root(
    claim: &ClaimRecordV1,
    normalized_evidence: &[ClaimEvidenceRef],
    normalized_relations: &[vela_protocol::claim_record::ClaimRelation],
    artifact_by_predecessor: &BTreeMap<&str, &ArtifactCompactionMapV1>,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct EvidenceProjection<'a> {
        relation: &'a str,
        content_root: String,
    }
    #[derive(Serialize)]
    struct ScientificProjection<'a> {
        schema: &'static str,
        assertion: &'a vela_protocol::claim_record::ClaimAssertion,
        conditions: &'a [String],
        evidence: Vec<EvidenceProjection<'a>>,
        provenance: &'a [vela_protocol::claim_record::ClaimSource],
        relations: &'a [vela_protocol::claim_record::ClaimRelation],
        standing: &'static str,
    }
    let evidence = claim
        .evidence
        .iter()
        .zip(normalized_evidence)
        .map(|(original, normalized)| {
            let content_root = original
                .artifact_id
                .as_deref()
                .and_then(|id| artifact_by_predecessor.get(id))
                .map_or_else(
                    || normalized.artifact_root.clone(),
                    |mapping| mapping.evidence_content_root.clone(),
                );
            EvidenceProjection {
                relation: &original.relation,
                content_root,
            }
        })
        .collect();
    let projection = ScientificProjection {
        schema: "vela.scientific-claim-projection.v1",
        assertion: &claim.assertion,
        conditions: &claim.conditions,
        evidence,
        provenance: &claim.provenance,
        relations: normalized_relations,
        standing: "accepted",
    };
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&projection)?
    ))
}

fn map_imported_artifact(
    frontier: &Path,
    reference: &RepositoryObjectRefV1,
    record: &CurrentArtifactRecordV1,
    record_bytes: &[u8],
) -> Result<ArtifactCompactionMapV1, String> {
    if reference.id != record.artifact_id || reference.root != record.canonical_root()? {
        return Err(format!(
            "Artifact {} does not match its repository reference",
            reference.id
        ));
    }
    let descriptor = record
        .artifact
        .as_object()
        .ok_or_else(|| format!("Artifact {} descriptor is not an object", reference.id))?;
    let storage_mode = descriptor
        .get("storage_mode")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("Artifact {} has no storage_mode", reference.id))?;
    let declared_content_root = descriptor
        .get("content_hash")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("Artifact {} has no content_hash", reference.id))?;
    require_sha256("Artifact content_hash", declared_content_root)?;

    let (candidate_artifact_root, form) = match storage_mode {
        "local_blob" => {
            let locator = descriptor
                .get("locator")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| format!("Artifact {} has no local locator", reference.id))?;
            let relative = Path::new(locator);
            if relative.is_absolute()
                || relative
                    .components()
                    .any(|component| matches!(component, Component::ParentDir))
            {
                return Err(format!(
                    "Artifact {} local locator is not a safe relative path",
                    reference.id
                ));
            }
            let content = fs::read(frontier.join(relative)).map_err(|error| {
                format!(
                    "read retained bytes for Artifact {} at {locator}: {error}",
                    reference.id
                )
            })?;
            let observed = format!("sha256:{}", hex::encode(Sha256::digest(&content)));
            if observed != declared_content_root {
                return Err(format!(
                    "Artifact {} retained bytes disagree with content_hash: declared {}, observed {observed}",
                    reference.id, declared_content_root
                ));
            }
            (observed, CompactedArtifactForm::LocalBlob)
        }
        "remote" | "pointer" => {
            let observed = format!("sha256:{}", hex::encode(Sha256::digest(record_bytes)));
            if observed != reference.root {
                return Err(format!(
                    "Artifact {} remote wrapper bytes disagree with repository root",
                    reference.id
                ));
            }
            (observed, CompactedArtifactForm::ExternalReference)
        }
        other => {
            return Err(format!(
                "Artifact {} has unsupported storage_mode `{other}`",
                reference.id
            ));
        }
    };
    let candidate_artifact_id = candidate_artifact_root
        .strip_prefix("sha256:")
        .expect("candidate root is constructed above")
        .to_string();
    let evidence_content_root = match form {
        CompactedArtifactForm::LocalBlob => declared_content_root.to_string(),
        CompactedArtifactForm::ExternalReference => reference.root.clone(),
    };

    Ok(ArtifactCompactionMapV1 {
        predecessor_artifact_id: reference.id.clone(),
        predecessor_record_root: reference.root.clone(),
        evidence_content_root,
        candidate_artifact_id,
        candidate_artifact_root,
        form,
    })
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("{field} must be a full sha256: digest"))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{field} must be a full sha256: digest"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn root(bytes: &[u8]) -> String {
        format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
    }

    fn reference(id: &str, bytes: &[u8]) -> RepositoryObjectRefV1 {
        RepositoryObjectRefV1 {
            schema: CURRENT_ARTIFACT_RECORD_SCHEMA_V1.into(),
            id: id.into(),
            root: root(bytes),
            path: format!("records/artifacts/sha256/{}.json", &root(bytes)[7..]),
        }
    }

    fn record(id: &str, storage_mode: &str, locator: &str, content_root: &str) -> Vec<u8> {
        let value = serde_json::json!({
            "schema": CURRENT_ARTIFACT_RECORD_SCHEMA_V1,
            "artifact_id": id,
            "artifact": {
                "id": id,
                "name": "Fixture evidence",
                "kind": "dataset",
                "media_type": "application/octet-stream",
                "content_hash": content_root,
                "size_bytes": 7,
                "license": "CC0-1.0",
                "access_tier": "public",
                "storage_mode": storage_mode,
                "locator": locator,
                "created": "2026-07-29T00:00:00Z",
                "provenance": {},
                "metadata": {},
                "target_findings": [],
                "retracted": false
            },
            "imported_object_root": format!("sha256:{}", "1".repeat(64)),
            "predecessor_commit": "2".repeat(40)
        });
        vela_protocol::canonical::to_canonical_bytes(&value).unwrap()
    }

    #[test]
    fn local_blob_maps_to_its_exact_content_hash() {
        let directory = tempdir().unwrap();
        let content = b"fixture";
        let locator = ".vela/artifact-blobs/sha256/fixture";
        let path = directory.path().join(locator);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        let bytes = record("va_fixture", "local_blob", locator, &root(content));
        let reference = reference("va_fixture", &bytes);
        let record = CurrentArtifactRecordV1::parse(&bytes).unwrap();

        let mapping = map_imported_artifact(directory.path(), &reference, &record, &bytes).unwrap();
        assert_eq!(mapping.candidate_artifact_root, root(content));
        assert_eq!(mapping.form, CompactedArtifactForm::LocalBlob);
    }

    #[test]
    fn remote_descriptor_maps_to_its_exact_retained_bytes() {
        let directory = tempdir().unwrap();
        let bytes = record(
            "va_fixture",
            "remote",
            "https://example.test/artifact",
            &format!("sha256:{}", "3".repeat(64)),
        );
        let reference = reference("va_fixture", &bytes);
        let record = CurrentArtifactRecordV1::parse(&bytes).unwrap();

        let mapping = map_imported_artifact(directory.path(), &reference, &record, &bytes).unwrap();
        assert_eq!(mapping.candidate_artifact_root, reference.root);
        assert_eq!(mapping.form, CompactedArtifactForm::ExternalReference);
    }

    #[test]
    fn missing_or_substituted_local_bytes_fail_closed() {
        let directory = tempdir().unwrap();
        let locator = ".vela/artifact-blobs/sha256/missing";
        let bytes = record(
            "va_fixture",
            "local_blob",
            locator,
            &format!("sha256:{}", "4".repeat(64)),
        );
        let reference = reference("va_fixture", &bytes);
        let record = CurrentArtifactRecordV1::parse(&bytes).unwrap();
        assert!(
            map_imported_artifact(directory.path(), &reference, &record, &bytes)
                .unwrap_err()
                .contains("read retained bytes")
        );

        let path = directory.path().join(locator);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"substituted").unwrap();
        assert!(
            map_imported_artifact(directory.path(), &reference, &record, &bytes)
                .unwrap_err()
                .contains("disagree with content_hash")
        );
    }
}
