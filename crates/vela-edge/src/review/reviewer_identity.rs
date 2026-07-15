//! Role-scoped scientific attestations.
//!
//! These records are local review artifacts. They state that a named
//! reviewer attested a bounded target under a declared scope. They do
//! not imply global consensus or institutional multi-signature approval.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use vela_protocol::repo;

/// Resolve the local `.vela/` repository root for a frontier path.
/// Scientific attestations require a local repo; project files and
/// packet directories are rejected.
fn repo_root(frontier_path: &Path) -> Result<PathBuf, String> {
    match repo::detect(frontier_path)? {
        repo::VelaSource::VelaRepo(root) => Ok(root),
        repo::VelaSource::ProjectFile(_) | repo::VelaSource::PacketDir(_) => Err(format!(
            "scientific attestations require a local .vela/ repository; got {}",
            frontier_path.display()
        )),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AttestationScope {
    SourceExtraction,
    MethodReview,
    StatisticalReview,
    DomainRelevance,
    TranslationClarity,
    PolicyApproval,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewerIdentity {
    pub reviewer_id: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orcid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ror: Option<String>,
    #[serde(default)]
    pub declared_scopes: Vec<AttestationScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScientificAttestation {
    pub schema: String,
    pub attestation_id: String,
    pub target_id: String,
    pub target_kind: String,
    pub reviewer: ReviewerIdentity,
    pub reason: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_event_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

pub fn list(frontier_path: &Path) -> Result<Vec<ScientificAttestation>, String> {
    let root = repo_root(frontier_path)?;
    let dir = attestations_dir(&root);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("read attestations dir: {e}"))? {
        let path = entry
            .map_err(|e| format!("read attestation entry: {e}"))?
            .path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let data = std::fs::read_to_string(&path)
                .map_err(|e| format!("read attestation {}: {e}", path.display()))?;
            let attestation: ScientificAttestation =
                serde_json::from_str(&data).map_err(|e| format!("parse attestation: {e}"))?;
            out.push(attestation);
        }
    }
    out.sort_by(|a, b| a.attestation_id.cmp(&b.attestation_id));
    Ok(out)
}

pub fn attestations_for_target(
    frontier_path: &Path,
    target_id: &str,
) -> Result<Vec<ScientificAttestation>, String> {
    Ok(list(frontier_path)?
        .into_iter()
        .filter(|attestation| attestation.target_id == target_id)
        .collect())
}

pub fn missing_roles_for_target(
    frontier_path: &Path,
    target_id: &str,
    required_roles: &[String],
) -> Result<Vec<String>, String> {
    let attestations = attestations_for_target(frontier_path, target_id)?;
    let mut missing = Vec::new();
    for role in required_roles {
        if !attestations
            .iter()
            .any(|attestation| attestation.reviewer.role == *role)
        {
            missing.push(role.clone());
        }
    }
    missing.sort();
    missing.dedup();
    Ok(missing)
}

fn attestations_dir(root: &Path) -> PathBuf {
    root.join(".vela").join("attestations")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn read_path_lists_attestations_and_reports_missing_roles() {
        let root = tempdir().expect("temporary frontier");
        let dir = root.path().join(".vela/attestations");
        std::fs::create_dir_all(&dir).expect("attestations directory");
        std::fs::write(
            dir.join("vatt_domain.json"),
            serde_json::json!({
                "schema": "vela.scientific_attestation.v0.1",
                "attestation_id": "vatt_domain",
                "target_id": "vsd_demo",
                "target_kind": "diff_pack",
                "reviewer": {
                    "reviewer_id": "reviewer:domain-one",
                    "role": "domain_reviewer",
                    "declared_scopes": ["domain_relevance"]
                },
                "reason": "bounded domain review",
                "created_at": "2026-07-15T00:00:00Z"
            })
            .to_string(),
        )
        .expect("attestation record");

        let records = list(root.path()).expect("list attestations");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].reviewer.role, "domain_reviewer");
        assert_eq!(
            missing_roles_for_target(
                root.path(),
                "vsd_demo",
                &["method_reviewer".to_string(), "domain_reviewer".to_string()],
            )
            .expect("missing roles"),
            vec!["method_reviewer"]
        );
    }
}
