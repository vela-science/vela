//! Exact scientific-projection equivalence for one pre-release compaction.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CURRENT_STATE_EQUIVALENCE_V1_SCHEMA: &str = "vela.current-state-equivalence.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactedArtifactForm {
    LocalBlob,
    ExternalReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactCompactionMapV1 {
    pub predecessor_artifact_id: String,
    pub predecessor_record_root: String,
    pub evidence_content_root: String,
    pub candidate_artifact_id: String,
    pub candidate_artifact_root: String,
    pub form: CompactedArtifactForm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimCompactionMapV1 {
    pub predecessor_claim_id: String,
    pub predecessor_claim_root: String,
    pub candidate_claim_id: String,
    pub candidate_claim_root: String,
    pub standing: String,
    pub predecessor_projection_root: String,
    pub candidate_projection_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentStateEquivalenceV1 {
    pub schema: String,
    pub equivalence_report_id: String,
    pub frontier_id: String,
    pub predecessor_repository_root: String,
    pub candidate_object_set_root: String,
    pub artifact_map: Vec<ArtifactCompactionMapV1>,
    pub claim_map: Vec<ClaimCompactionMapV1>,
    pub archived_live_object_roots: Vec<String>,
    pub accepted_count_before: u64,
    pub accepted_count_after: u64,
    pub relation_count_before: u64,
    pub relation_count_after: u64,
    pub equivalent: bool,
}

impl CurrentStateEquivalenceV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        frontier_id: String,
        predecessor_repository_root: String,
        candidate_object_set_root: String,
        artifact_map: Vec<ArtifactCompactionMapV1>,
        claim_map: Vec<ClaimCompactionMapV1>,
        archived_live_object_roots: Vec<String>,
        accepted_count_before: u64,
        accepted_count_after: u64,
        relation_count_before: u64,
        relation_count_after: u64,
    ) -> Result<Self, String> {
        let mut value = Self {
            schema: CURRENT_STATE_EQUIVALENCE_V1_SCHEMA.into(),
            equivalence_report_id: String::new(),
            frontier_id,
            predecessor_repository_root,
            candidate_object_set_root,
            artifact_map,
            claim_map,
            archived_live_object_roots,
            accepted_count_before,
            accepted_count_after,
            relation_count_before,
            relation_count_after,
            equivalent: true,
        };
        value.validate_semantics()?;
        value.equivalence_report_id = value.derive_id()?;
        value.verify()?;
        Ok(value)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("current-state equivalence report exceeds 16 MiB".into());
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("parse current-state equivalence v1: {error}"))?;
        value.verify()?;
        if value.canonical_bytes()? != bytes {
            return Err("current-state equivalence bytes are not canonical JSON".into());
        }
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        self.validate_semantics()?;
        let expected = self.derive_id()?;
        if self.equivalence_report_id != expected {
            return Err(format!(
                "current-state equivalence id mismatch: declared {}, rebuilt {expected}",
                self.equivalence_report_id
            ));
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_semantics()?;
        crate::canonical::to_canonical_bytes(self)
    }

    pub fn canonical_root(&self) -> Result<String, String> {
        Ok(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(self.canonical_bytes()?))
        ))
    }

    fn derive_id(&self) -> Result<String, String> {
        let mut body = self.clone();
        body.equivalence_report_id.clear();
        let bytes = crate::canonical::to_canonical_bytes(&body)?;
        Ok(format!("vce_{}", &hex::encode(Sha256::digest(bytes))[..16]))
    }

    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != CURRENT_STATE_EQUIVALENCE_V1_SCHEMA {
            return Err(format!(
                "current-state equivalence schema must be `{CURRENT_STATE_EQUIVALENCE_V1_SCHEMA}`"
            ));
        }
        require_prefixed("frontier_id", &self.frontier_id, "vfr_")?;
        require_sha256(
            "predecessor_repository_root",
            &self.predecessor_repository_root,
        )?;
        require_sha256("candidate_object_set_root", &self.candidate_object_set_root)?;
        if !self.equivalent {
            return Err(
                "current-state equivalence report must fail closed, not encode false".into(),
            );
        }
        if self.accepted_count_before != self.accepted_count_after
            || self.accepted_count_before != self.claim_map.len() as u64
        {
            return Err(
                "current-state equivalence accepted counts must match the total Claim map".into(),
            );
        }
        if self.relation_count_before != self.relation_count_after {
            return Err("current-state equivalence relation counts differ".into());
        }

        let mut previous_artifact_id = None;
        let mut predecessor_artifacts = BTreeSet::new();
        let mut candidate_artifacts = BTreeSet::new();
        for mapping in &self.artifact_map {
            require_prefixed(
                "artifact_map.predecessor_artifact_id",
                &mapping.predecessor_artifact_id,
                "va_",
            )?;
            require_sha256(
                "artifact_map.predecessor_record_root",
                &mapping.predecessor_record_root,
            )?;
            require_sha256(
                "artifact_map.evidence_content_root",
                &mapping.evidence_content_root,
            )?;
            require_content_id(
                "artifact_map.candidate_artifact_id",
                &mapping.candidate_artifact_id,
            )?;
            require_sha256(
                "artifact_map.candidate_artifact_root",
                &mapping.candidate_artifact_root,
            )?;
            if mapping.candidate_artifact_root
                != format!("sha256:{}", mapping.candidate_artifact_id)
            {
                return Err(
                    "current-state equivalence candidate Artifact ID must equal its byte root"
                        .into(),
                );
            }
            if previous_artifact_id
                .as_ref()
                .is_some_and(|previous| previous >= &mapping.predecessor_artifact_id)
            {
                return Err(
                    "current-state equivalence Artifact map must be uniquely sorted by predecessor ID"
                        .into(),
                );
            }
            previous_artifact_id = Some(mapping.predecessor_artifact_id.clone());
            if !predecessor_artifacts.insert(mapping.predecessor_artifact_id.clone())
                || !candidate_artifacts.insert(mapping.candidate_artifact_id.clone())
            {
                return Err("current-state equivalence Artifact map must be one-to-one".into());
            }
        }

        let mut previous_claim_id = None;
        let mut predecessor_claims = BTreeSet::new();
        let mut candidate_claims = BTreeSet::new();
        for mapping in &self.claim_map {
            require_full_claim_id(
                "claim_map.predecessor_claim_id",
                &mapping.predecessor_claim_id,
            )?;
            require_sha256(
                "claim_map.predecessor_claim_root",
                &mapping.predecessor_claim_root,
            )?;
            require_full_claim_id("claim_map.candidate_claim_id", &mapping.candidate_claim_id)?;
            require_sha256(
                "claim_map.candidate_claim_root",
                &mapping.candidate_claim_root,
            )?;
            if mapping.standing != "accepted" {
                return Err(
                    "current-state equivalence Claim map may contain only accepted Standing".into(),
                );
            }
            require_sha256(
                "claim_map.predecessor_projection_root",
                &mapping.predecessor_projection_root,
            )?;
            require_sha256(
                "claim_map.candidate_projection_root",
                &mapping.candidate_projection_root,
            )?;
            if mapping.predecessor_projection_root != mapping.candidate_projection_root {
                return Err("current-state equivalence Claim scientific projections differ".into());
            }
            if previous_claim_id
                .as_ref()
                .is_some_and(|previous| previous >= &mapping.predecessor_claim_id)
            {
                return Err(
                    "current-state equivalence Claim map must be uniquely sorted by predecessor ID"
                        .into(),
                );
            }
            previous_claim_id = Some(mapping.predecessor_claim_id.clone());
            if !predecessor_claims.insert(mapping.predecessor_claim_id.clone())
                || !candidate_claims.insert(mapping.candidate_claim_id.clone())
            {
                return Err("current-state equivalence Claim map must be bijective".into());
            }
        }

        let mut previous_root = None;
        for root in &self.archived_live_object_roots {
            require_sha256("archived_live_object_roots", root)?;
            if previous_root
                .as_ref()
                .is_some_and(|previous| previous >= root)
            {
                return Err(
                    "current-state equivalence archived roots must be unique and sorted".into(),
                );
            }
            previous_root = Some(root.clone());
        }
        Ok(())
    }
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "current-state equivalence {field} must be non-empty, trimmed text"
        ));
    }
    Ok(())
}

fn require_prefixed(field: &str, value: &str, prefix: &str) -> Result<(), String> {
    require_text(field, value)?;
    if !value.starts_with(prefix) {
        return Err(format!(
            "current-state equivalence {field} must start with `{prefix}`"
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    let digest = value.strip_prefix("sha256:").ok_or_else(|| {
        format!("current-state equivalence {field} must be a full sha256: digest")
    })?;
    require_content_id(field, digest)
}

fn require_content_id(field: &str, value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "current-state equivalence {field} must be a full lowercase content hash"
        ));
    }
    Ok(())
}

fn require_full_claim_id(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("vcl_")
        .ok_or_else(|| format!("current-state equivalence {field} must be a full vcl_ digest"))?;
    require_content_id(field, digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn artifact_map() -> Vec<ArtifactCompactionMapV1> {
        vec![ArtifactCompactionMapV1 {
            predecessor_artifact_id: "va_fixture".into(),
            predecessor_record_root: root('1'),
            evidence_content_root: root('2'),
            candidate_artifact_id: "3".repeat(64),
            candidate_artifact_root: root('3'),
            form: CompactedArtifactForm::LocalBlob,
        }]
    }

    fn claim_map() -> Vec<ClaimCompactionMapV1> {
        vec![ClaimCompactionMapV1 {
            predecessor_claim_id: format!("vcl_{}", "4".repeat(64)),
            predecessor_claim_root: root('5'),
            candidate_claim_id: format!("vcl_{}", "6".repeat(64)),
            candidate_claim_root: root('7'),
            standing: "accepted".into(),
            predecessor_projection_root: root('8'),
            candidate_projection_root: root('8'),
        }]
    }

    fn fixture() -> CurrentStateEquivalenceV1 {
        CurrentStateEquivalenceV1::build(
            "vfr_fixture".into(),
            root('a'),
            root('b'),
            artifact_map(),
            claim_map(),
            vec![root('c')],
            1,
            1,
            0,
            0,
        )
        .unwrap()
    }

    #[test]
    fn exact_bijective_projection_passes() {
        let report = fixture();
        CurrentStateEquivalenceV1::parse(&report.canonical_bytes().unwrap()).unwrap();
    }

    #[test]
    fn standing_count_drift_fails_closed() {
        let mut report = fixture();
        report.accepted_count_after = 0;
        assert!(report.verify().is_err());
    }

    #[test]
    fn scientific_projection_drift_fails_closed() {
        let mut report = fixture();
        report.claim_map[0].candidate_projection_root = root('9');
        assert!(report.verify().is_err());
    }

    #[test]
    fn candidate_artifact_substitution_fails_closed() {
        let mut report = fixture();
        report.artifact_map[0].candidate_artifact_id = "f".repeat(64);
        assert!(report.verify().is_err());
    }

    #[test]
    fn reordered_maps_fail_closed() {
        let mut report = fixture();
        let mut later = report.claim_map[0].clone();
        later.predecessor_claim_id = format!("vcl_{}", "3".repeat(64));
        later.candidate_claim_id = format!("vcl_{}", "9".repeat(64));
        report.claim_map.push(later);
        report.accepted_count_before = 2;
        report.accepted_count_after = 2;
        assert!(report.verify().is_err());
    }
}
