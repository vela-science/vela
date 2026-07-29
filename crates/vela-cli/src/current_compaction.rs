//! Read-only preparation for the one-time pre-release repository compaction.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

use serde::Serialize;
use sha2::{Digest, Sha256};
use vela_protocol::claim_record::{ClaimEvidenceRef, ClaimRecordV1, LEGACY_FINDING_EXTENSION};
use vela_protocol::current_repository::{
    CURRENT_ARTIFACT_RECORD_SCHEMA_V1, CurrentArtifactRecordV1, RepositoryObjectRefV1,
};
use vela_protocol::current_state_equivalence::{
    ArtifactCompactionMapV1, ClaimCompactionMapV1, CompactedArtifactForm, CurrentStateEquivalenceV1,
};

struct ClaimCompactionAudit {
    report: CurrentStateEquivalenceV1,
    claim_map_root: String,
}

pub(crate) fn cmd_compaction_check(frontier: &Path, check: bool, json_out: bool) {
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
        "next_action": "bind the exact candidate files and repository-origin plan before any authority action",
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
        println!("  writes: no");
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
    })
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
