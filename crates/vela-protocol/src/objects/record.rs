//! Activity records (v0): the portable claim packet — `vela record`.
//!
//! A record is a structured PROPOSAL to change frontier state — emitted by
//! any workbench (an AI agent, a notebook, an HPC job, a lab system),
//! carried anywhere (a PR, an email, an artifact store), and landed on a
//! frontier as a pending proposal for a human key to accept. It lives in
//! the ACTIVITY plane ("activity is not state"; the claim-centric sibling
//! of the action-centric `ActivityEnvelope`): a record is NOT truth; it is
//! activity shaped so the merge layer can judge it.
//!
//! Design, deliberately git-small:
//! - content-addressed (`vrc_` + sha256(canonical body, id="")[:16]) so a
//!   receipt is immutable and citable the moment it exists;
//! - frontier-pinned: it names the `vfr_` it proposes against AND the
//!   `event_log_hash` head it was emitted against, so a reviewer sees
//!   exactly how stale it is (the decision-delta);
//! - evidence-bound: every artifact ref carries a sha256 the validator
//!   re-derives from bytes, so a receipt can't cite what it can't show;
//! - signature optional at emit (an agent without a key may still emit;
//!   `signed=false` is loud), MANDATORY judgment at accept (the human key,
//!   as everywhere in Vela). Trust enters at the gate, not the emitter.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::bundle::{ArtifactAvailability, ArtifactDisclosure, LocatorIntegrity};
use crate::receipt_v1::ReceiptLineage;

pub const ACTIVITY_RECORD_SCHEMA: &str = "vela.activity-record.v0.1";

/// One evidence artifact the claim rests on. `locator` is where the bytes
/// live (a path relative to the receipt, a URL, a content-addressed blob);
/// `sha256` is what makes the reference binding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordArtifact {
    /// What kind of artifact: `witness`, `log`, `dataset`, `notebook`,
    /// `proof`, `analysis` — free-form, one word.
    pub kind: String,
    pub locator: String,
    /// sha256 (hex) of public artifact bytes. Empty for a restricted opaque
    /// custodian reference; restricted low-entropy bytes never get a public
    /// equality digest merely to satisfy this compatibility index.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "ArtifactDisclosure::is_unknown")]
    pub disclosure: ArtifactDisclosure,
    #[serde(default, skip_serializing_if = "LocatorIntegrity::is_unknown")]
    pub locator_integrity: LocatorIntegrity,
    #[serde(default, skip_serializing_if = "ArtifactAvailability::is_unknown")]
    pub availability: ArtifactAvailability,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// A verifier run the emitter already performed (mechanical provenance,
/// not a verdict): `method` names the verifier, `outcome` its result,
/// `output_hash` content-addresses its output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordVerifierRun {
    pub method: String,
    pub outcome: String,
    pub output_hash: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub solver: String,
}

/// External source-of-record metadata for a landed activity record.
/// The emitting machine remains `emitted_by`; this names the scientific
/// producer whose output the record carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RecordSource {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub uri: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityRecord {
    pub schema: String,
    /// Content-addressed id: `vrc_` + sha256(canonical body, id = "")[:16].
    pub id: String,
    /// The frontier this proposes against.
    pub frontier_id: String,
    /// The frontier head (`event_log_hash`) at emit time — the staleness pin.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub against_head: String,
    /// The claim: what the emitter asserts is now known / bounded / refuted.
    pub assertion: String,
    /// Claim type, mirroring finding types: `theoretical`, `computational`,
    /// `empirical`, `negative`.
    pub assertion_type: String,
    pub artifacts: Vec<RecordArtifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verifier_runs: Vec<RecordVerifierRun>,
    /// What this claim does NOT establish. Required non-empty: a receipt
    /// with no stated limits is advertising, not science.
    pub caveats: Vec<String>,
    /// Scientific source of record. Optional for legacy records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<RecordSource>,
    /// External references the proposal should cite. They are descriptive
    /// provenance and may include content-addressed artifact refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    /// Canonical digest of the complete logical receipt supplied to `land`.
    /// It binds review input but carries no acceptance authority.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub receipt_digest: String,
    /// Frontier-relative locator of the canonical, lossless Receipt v1 bytes.
    /// New records use this pointer as their evidence source of truth; the
    /// older flattened fields remain a deterministic compatibility index.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub receipt_path: String,
    /// Client operation identity that made the record durable. This separates
    /// an exact retry from a same-claim receipt carrying independent evidence.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub operation_id: String,
    /// Producer-declared lineage copied from the receipt for review. The gate
    /// may validate it; its presence never makes the claim accepted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage: Option<ReceiptLineage>,
    /// The `vrc_` id of the record revision this one supersedes (optional;
    /// absent on legacy records, so their ids are byte-unchanged). Records
    /// are content-addressed — a revision mints a new id — so continuity
    /// across revisions is a back-pointer chain, the same affordance as
    /// `FindingBundle.previous_version` and `ScientificDiffPack.parent_pack`.
    /// Deliberate substitute for a stable-id + revision-digest pairing;
    /// see docs/adr/0002.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    /// Who emitted (agent:…, ci:…, reviewer:…). Agents welcome — emitting
    /// is proposing, never deciding.
    pub emitted_by: String,
    pub emitted_at: String,
    /// Ed25519 over the canonical body with `signature` empty. OPTIONAL:
    /// an unsigned receipt is still validatable and landable; `validate`
    /// reports `signed=false` loudly.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signer_pubkey_hex: String,
}

pub struct ActivityRecordDraft {
    pub frontier_id: String,
    pub against_head: String,
    pub assertion: String,
    pub assertion_type: String,
    pub artifacts: Vec<RecordArtifact>,
    pub verifier_runs: Vec<RecordVerifierRun>,
    pub caveats: Vec<String>,
    pub source: Option<RecordSource>,
    pub source_refs: Vec<String>,
    pub receipt_digest: String,
    pub receipt_path: String,
    pub operation_id: String,
    pub lineage: Option<ReceiptLineage>,
    pub emitted_by: String,
    pub emitted_at: String,
}

impl ActivityRecord {
    /// Build and content-address; sign iff a key is supplied.
    pub fn build(
        draft: ActivityRecordDraft,
        key: Option<&ed25519_dalek::SigningKey>,
    ) -> Result<Self, String> {
        if draft.assertion.trim().is_empty() {
            return Err("record assertion cannot be empty".to_string());
        }
        if !draft.frontier_id.starts_with("vfr_") {
            return Err(format!(
                "record frontier_id must be a vfr_… id, got '{}'",
                draft.frontier_id
            ));
        }
        if draft.artifacts.is_empty() {
            return Err("a record with no artifacts is a slogan; attach at least one".to_string());
        }
        for atom in &draft.artifacts {
            match atom.disclosure {
                ArtifactDisclosure::Restricted => {
                    if !atom.sha256.is_empty() {
                        return Err(format!(
                            "restricted evidence '{}' must not expose a public equality digest",
                            atom.locator
                        ));
                    }
                    if !(atom.locator.starts_with("custodian:")
                        || atom.locator.starts_with("opaque:"))
                    {
                        return Err(format!(
                            "restricted evidence '{}' needs an opaque custodian: or opaque: locator",
                            atom.locator
                        ));
                    }
                }
                ArtifactDisclosure::Public | ArtifactDisclosure::Unknown => {
                    if atom.sha256.len() != 64
                        || !atom
                            .sha256
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                    {
                        return Err(format!(
                            "evidence '{}' sha256 must be 32 bytes of lowercase hex",
                            atom.locator
                        ));
                    }
                }
            }
        }
        if draft.caveats.iter().all(|c| c.trim().is_empty()) {
            return Err(
                "a record must state at least one caveat (what this does NOT establish)"
                    .to_string(),
            );
        }
        if draft.emitted_by.trim().is_empty() {
            return Err("emitted_by is required (agent:…, ci:…, or reviewer:…)".to_string());
        }
        if !draft.receipt_digest.is_empty()
            && !draft
                .receipt_digest
                .strip_prefix("sha256:")
                .is_some_and(|digest| {
                    digest.len() == 64
                        && digest
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
        {
            return Err("receipt_digest must be sha256:<64 lowercase hex>".to_string());
        }
        if !draft.operation_id.is_empty()
            && !draft
                .operation_id
                .strip_prefix("vop_")
                .is_some_and(|digest| {
                    digest.len() == 64
                        && digest
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
        {
            return Err("operation_id must be vop_<64 lowercase hex>".to_string());
        }
        if !draft.receipt_path.is_empty() {
            let path = std::path::Path::new(&draft.receipt_path);
            if path.is_absolute()
                || path.components().any(|component| {
                    matches!(
                        component,
                        std::path::Component::ParentDir
                            | std::path::Component::RootDir
                            | std::path::Component::Prefix(_)
                    )
                })
            {
                return Err("receipt_path must be a normalized frontier-relative path".to_string());
            }
        }
        let mut rc = ActivityRecord {
            schema: ACTIVITY_RECORD_SCHEMA.to_string(),
            id: String::new(),
            frontier_id: draft.frontier_id,
            against_head: draft.against_head,
            assertion: draft.assertion,
            assertion_type: draft.assertion_type,
            artifacts: draft.artifacts,
            verifier_runs: draft.verifier_runs,
            caveats: draft.caveats,
            source: draft.source,
            source_refs: draft.source_refs,
            receipt_digest: draft.receipt_digest,
            receipt_path: draft.receipt_path,
            operation_id: draft.operation_id,
            lineage: draft.lineage,
            supersedes: None,
            emitted_by: draft.emitted_by,
            emitted_at: draft.emitted_at,
            signature: String::new(),
            signer_pubkey_hex: key
                .map(|k| hex::encode(k.verifying_key().to_bytes()))
                .unwrap_or_default(),
        };
        rc.id = rc.derive_id()?;
        if let Some(k) = key {
            use ed25519_dalek::Signer;
            rc.signature = hex::encode(k.sign(&rc.signing_bytes()?).to_bytes());
        }
        Ok(rc)
    }

    /// Canonical bytes with `signature` cleared (the id is signed content;
    /// the signature is not part of the id).
    pub fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut c = self.clone();
        c.signature = String::new();
        let body = crate::canonical::to_canonical_bytes(&c)?;
        Ok(crate::signing_input::signing_input(
            crate::signing_input::SigVersion::V0,
            crate::signing_input::payload_type::ACTIVITY_RECORD,
            &body,
        ))
    }

    pub fn derive_id(&self) -> Result<String, String> {
        let mut c = self.clone();
        c.id = String::new();
        c.signature = String::new();
        let bytes = crate::canonical::to_canonical_bytes(&c)?;
        Ok(format!("vrc_{}", &hex::encode(Sha256::digest(bytes))[..16]))
    }

    /// Shape this record into the standard `finding.add` proposal draft.
    /// The emitter is the acting originator. When source metadata is present,
    /// scientific authorship comes from the external source of record.
    pub fn to_finding_draft(
        &self,
        staleness: &str,
        signed: bool,
    ) -> crate::state::FindingDraftOptions {
        let conditions = format!(
            "Record {} ({}; {}). Receipt: {}. Caveats: {}. Artifacts: {} hash-verified at propose.",
            self.id,
            if signed { "signed" } else { "unsigned" },
            staleness,
            if self.receipt_digest.is_empty() {
                "legacy-unbound"
            } else {
                self.receipt_digest.as_str()
            },
            self.caveats.join(" | "),
            self.artifacts.len(),
        );
        let source = self.source.as_ref();
        let source_label = source
            .and_then(|s| (!s.name.trim().is_empty()).then(|| s.name.clone()))
            .unwrap_or_else(|| format!("record:{}", self.id));
        let source_type = source
            .and_then(|s| (!s.source_type.trim().is_empty()).then(|| s.source_type.clone()))
            .unwrap_or_else(|| "model_output".to_string());
        let url = source.and_then(|s| (!s.uri.trim().is_empty()).then(|| s.uri.clone()));
        let source_authors = source
            .map(|s| s.authors.clone())
            .filter(|authors| !authors.is_empty())
            .unwrap_or_else(|| vec![self.emitted_by.clone()]);
        let mut source_refs = self.source_refs.clone();
        source_refs.push(format!("record:{}", self.id));
        if !self.receipt_digest.is_empty() {
            source_refs.push(self.receipt_digest.clone());
        }
        if let Some(lineage) = &self.lineage {
            source_refs.extend(
                lineage
                    .parents
                    .iter()
                    .map(|parent| format!("parent:{parent}")),
            );
            source_refs.extend(lineage.source_refs.iter().cloned());
        }
        source_refs.sort();
        source_refs.dedup();
        // A Receipt artifact is already the producer's explicit evidence
        // binding. Preserve that relation as a typed finding span instead of
        // emitting a finding whose evidence locator is empty. The span points
        // into the retained canonical Receipt when available and carries only
        // the artifact locator/digest the record already exposes. It does not
        // infer a verifier pass, independence, or acceptance.
        let evidence_spans = self
            .artifacts
            .iter()
            .enumerate()
            .map(|(index, artifact)| {
                let source = if !self.receipt_path.is_empty() {
                    self.receipt_path.as_str()
                } else if !self.receipt_digest.is_empty() {
                    self.receipt_digest.as_str()
                } else {
                    artifact.locator.as_str()
                };
                let mut span = serde_json::json!({
                    "source": source,
                    "section": format!("artifacts[{index}]"),
                    "text": self.assertion.as_str(),
                    "artifact_kind": artifact.kind.as_str(),
                    "artifact_locator": artifact.locator.as_str(),
                });
                if !artifact.sha256.is_empty() {
                    span["artifact_sha256"] = serde_json::json!(artifact.sha256.as_str());
                }
                span
            })
            .collect();
        crate::state::FindingDraftOptions {
            text: self.assertion.clone(),
            assertion_type: self.assertion_type.clone(),
            source: source_label,
            source_type,
            author: self.emitted_by.clone(),
            confidence: 0.3,
            evidence_type: self.assertion_type.clone(),
            doi: None,
            year: None,
            url,
            source_authors,
            source_refs,
            conditions_text: Some(conditions),
            evidence_spans,
            gap: false,
            negative_space: false,
            replication_attestation: None,
        }
    }

    /// Build the pending finding proposal that indexes this record while
    /// retaining the canonical receipt as the evidence source of truth.
    ///
    /// The `vela_submission` block carries typed roots and relations only; it
    /// does not flatten a second receipt body into the proposal. The injected
    /// timestamp lets a transaction stage deterministic bytes before its
    /// durable marker.
    pub fn to_finding_proposal_at(
        &self,
        staleness: &str,
        signed: bool,
        at: &str,
    ) -> Result<crate::proposals::StateProposal, String> {
        let mut proposal = crate::state::build_add_finding_proposal_at(
            self.to_finding_draft(staleness, signed),
            at,
        )?;
        let payload = proposal
            .payload
            .as_object_mut()
            .ok_or_else(|| "finding proposal payload must be an object".to_string())?;
        payload.insert(
            "vela_submission".to_string(),
            serde_json::json!({
                "schema": "vela.submission-links.internal.v1",
                "receipt_root": self.receipt_digest,
                "receipt_path": self.receipt_path,
                "record_id": self.id,
                "operation_id": self.operation_id,
            }),
        );
        if !self.receipt_path.is_empty()
            && !proposal
                .source_refs
                .iter()
                .any(|item| item == &self.receipt_path)
        {
            proposal.source_refs.push(self.receipt_path.clone());
            proposal.source_refs.sort();
            proposal.source_refs.dedup();
        }
        proposal.id = crate::proposals::proposal_id(&proposal);
        Ok(proposal)
    }

    /// Full integrity check: schema, id re-derivation, namespace, and —
    /// when a signature is present — verification under the embedded
    /// pubkey. Returns whether the receipt is signed.
    pub fn verify(&self) -> Result<bool, String> {
        if self.schema != ACTIVITY_RECORD_SCHEMA {
            return Err(format!(
                "record schema must be {ACTIVITY_RECORD_SCHEMA}, got {}",
                self.schema
            ));
        }
        let derived = self.derive_id()?;
        if derived != self.id {
            return Err(format!(
                "record id does not re-derive: stored {}, derived {derived}",
                self.id
            ));
        }
        if self.signature.is_empty() {
            return Ok(false);
        }
        use ed25519_dalek::Verifier;
        let pk_bytes: [u8; 32] = hex::decode(&self.signer_pubkey_hex)
            .map_err(|e| format!("pubkey hex: {e}"))?
            .try_into()
            .map_err(|_| "pubkey must be 32 bytes".to_string())?;
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&pk_bytes)
            .map_err(|e| format!("pubkey: {e}"))?;
        let sig_bytes: [u8; 64] = hex::decode(&self.signature)
            .map_err(|e| format!("signature hex: {e}"))?
            .try_into()
            .map_err(|_| "signature must be 64 bytes".to_string())?;
        let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        vk.verify(&self.signing_bytes()?, &sig)
            .map_err(|_| "record signature does not verify".to_string())?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> ActivityRecordDraft {
        ActivityRecordDraft {
            frontier_id: "vfr_0123456789abcdef".into(),
            against_head: "sha256:abc".into(),
            assertion: "a(17) >= 292 for the Sidon frontier".into(),
            assertion_type: "computational".into(),
            artifacts: vec![RecordArtifact {
                kind: "witness".into(),
                locator: "witnesses/a17.json".into(),
                sha256: "a".repeat(64),
                size_bytes: None,
                media_type: None,
                disclosure: ArtifactDisclosure::Unknown,
                locator_integrity: LocatorIntegrity::Unknown,
                availability: ArtifactAvailability::Unknown,
                note: String::new(),
            }],
            verifier_runs: vec![],
            caveats: vec!["lower bound only; optimality not established".into()],
            source: None,
            source_refs: Vec::new(),
            receipt_digest: String::new(),
            receipt_path: String::new(),
            operation_id: String::new(),
            lineage: None,
            emitted_by: "agent:claude".into(),
            emitted_at: "2026-07-01T00:00:00Z".into(),
        }
    }

    fn key() -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[7u8; 32])
    }

    #[test]
    fn unsigned_record_builds_and_verifies_as_unsigned() {
        let r = ActivityRecord::build(draft(), None).unwrap();
        assert!(r.id.starts_with("vrc_"));
        assert!(!r.verify().unwrap());
    }

    #[test]
    fn receipt_pointer_and_operation_identity_are_bound_into_the_record() {
        let mut input = draft();
        input.receipt_digest = format!("sha256:{}", "b".repeat(64));
        input.receipt_path = "records/receipts/sha256/b.json".to_string();
        input.operation_id = format!("vop_{}", "c".repeat(64));
        let record = ActivityRecord::build(input, None).unwrap();
        assert_eq!(record.receipt_path, "records/receipts/sha256/b.json");
        assert!(record.operation_id.starts_with("vop_"));

        let mut tampered = record.clone();
        tampered.operation_id = format!("vop_{}", "d".repeat(64));
        assert!(tampered.verify().is_err());

        let proposal = record
            .to_finding_proposal_at("current head", false, "2026-07-13T00:00:00Z")
            .unwrap();
        let links = proposal.payload.get("vela_submission").unwrap();
        assert_eq!(links["receipt_root"], record.receipt_digest);
        assert_eq!(links["record_id"], record.id);
        assert!(!proposal.payload.to_string().contains("verifier_runs"));
        let span = &proposal.payload["finding"]["evidence"]["evidence_spans"][0];
        assert_eq!(span["source"], record.receipt_path);
        assert_eq!(span["section"], "artifacts[0]");
        assert_eq!(span["artifact_kind"], "witness");
        assert_eq!(span["artifact_locator"], "witnesses/a17.json");
        assert_eq!(span["artifact_sha256"], "a".repeat(64));
    }

    #[test]
    fn receipt_pointer_rejects_escape_and_untyped_operation_ids() {
        let mut escaped = draft();
        escaped.receipt_path = "../secret.json".to_string();
        assert!(ActivityRecord::build(escaped, None).is_err());

        let mut invalid_operation = draft();
        invalid_operation.operation_id = "vop_request".to_string();
        assert!(ActivityRecord::build(invalid_operation, None).is_err());
    }

    #[test]
    fn signed_record_verifies_and_tamper_fails() {
        let r = ActivityRecord::build(draft(), Some(&key())).unwrap();
        assert!(r.verify().unwrap());
        let mut bad = r.clone();
        bad.assertion = "a(17) >= 300".into();
        assert!(bad.verify().is_err()); // id no longer re-derives
    }

    #[test]
    fn record_without_artifacts_or_caveats_refused() {
        let mut d = draft();
        d.artifacts.clear();
        assert!(ActivityRecord::build(d, None).is_err());
        let mut d = draft();
        d.caveats = vec!["".into()];
        assert!(ActivityRecord::build(d, None).is_err());
    }

    #[test]
    fn receipt_binding_and_lineage_are_visible_to_review() {
        let mut d = draft();
        d.receipt_digest = format!("sha256:{}", "b".repeat(64));
        d.lineage = Some(ReceiptLineage {
            parents: vec!["vf_parent".into()],
            source_refs: vec!["urn:sha256:source".into()],
            ..ReceiptLineage::default()
        });
        let record = ActivityRecord::build(d, None).unwrap();
        let finding = record.to_finding_draft("recorded against head", false);
        assert!(finding.source_refs.contains(&record.receipt_digest));
        assert!(
            finding
                .source_refs
                .contains(&"parent:vf_parent".to_string())
        );
        assert!(
            finding
                .source_refs
                .contains(&format!("record:{}", record.id))
        );
    }

    #[test]
    fn restricted_artifact_span_preserves_opaque_locator_without_equality_digest() {
        let mut input = draft();
        input.artifacts[0].locator = "custodian:restricted-evidence-01".to_string();
        input.artifacts[0].sha256.clear();
        input.artifacts[0].disclosure = ArtifactDisclosure::Restricted;
        input.receipt_digest = format!("sha256:{}", "b".repeat(64));
        input.receipt_path = "records/receipts/sha256/restricted.json".to_string();
        let record = ActivityRecord::build(input, None).unwrap();

        let finding = record.to_finding_draft("current head", false);
        let span = &finding.evidence_spans[0];
        assert_eq!(span["source"], record.receipt_path);
        assert_eq!(span["artifact_locator"], "custodian:restricted-evidence-01");
        assert!(span.get("artifact_sha256").is_none());
    }
}
