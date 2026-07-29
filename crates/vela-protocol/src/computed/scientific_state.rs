//! Closed scientific-state root for Frontier Repository Profile v1.
//!
//! `vela.scientific-state.v2` commits only to the protocol collections named
//! here. Display metadata, operational state, authority records, the event log,
//! proposals, signatures, proof exports, and leases retain their own roots or
//! deliberately non-scientific roles. Adding a field to [`Project`] therefore
//! cannot silently change this security identity.

use serde::{Deserialize, Serialize};

use crate::bundle::{
    Annotation, Assertion, Attachment, Conditions, Confidence, Evidence, FindingBundle, Flags,
    Provenance,
};
use crate::project::Project;

pub const SCIENTIFIC_STATE_SCHEMA_V2: &str = "vela.scientific-state.v2";

/// The complete and closed root record for scientific state in Repository
/// Profile v1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScientificStateV2 {
    pub schema: String,
    pub identity_root: String,
    pub dependency_root: String,
    pub findings_root: String,
    pub sources_root: String,
    pub evidence_atoms_root: String,
    pub condition_records_root: String,
    pub review_events_root: String,
    pub confidence_updates_root: String,
    pub artifacts_root: String,
    pub released_diff_packs_root: String,
    pub verdict_conflicts_root: String,
    pub contradictions_root: String,
    pub verifier_attachments_root: String,
    pub attempts_root: String,
    pub attempt_resolutions_root: String,
    pub transfers_root: String,
    pub endorsements_root: String,
    pub statement_attestations_root: String,
    pub anchor_links_root: String,
    pub statement_registrations_root: String,
}

/// The closed scientific projection of one finding.
///
/// `FindingBundle` also carries mutable graph links and a read-side access
/// tier. Both are intentionally outside the finding content commitment and
/// outside scientific state: changing how a record is connected or disclosed
/// must not silently mint a different scientific snapshot. Keep this explicit
/// projection in lockstep with the protocol fields that actually describe the
/// assertion, evidence, conditions, interpretation, and provenance.
#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct ScientificFindingV2<'a> {
    id: &'a str,
    version: u32,
    previous_version: &'a Option<String>,
    assertion: &'a Assertion,
    evidence: &'a Evidence,
    conditions: &'a Conditions,
    confidence: &'a Confidence,
    provenance: &'a Provenance,
    flags: &'a Flags,
    annotations: &'a [Annotation],
    attachments: &'a [Attachment],
    created: &'a str,
    updated: &'a Option<String>,
}

impl<'a> From<&'a FindingBundle> for ScientificFindingV2<'a> {
    fn from(finding: &'a FindingBundle) -> Self {
        Self {
            id: &finding.id,
            version: finding.version,
            previous_version: &finding.previous_version,
            assertion: &finding.assertion,
            evidence: &finding.evidence,
            conditions: &finding.conditions,
            confidence: &finding.confidence,
            provenance: &finding.provenance,
            flags: &finding.flags,
            annotations: &finding.annotations,
            attachments: &finding.attachments,
            created: &finding.created,
            updated: &finding.updated,
        }
    }
}

impl ScientificStateV2 {
    /// Derive the closed component-root record from a materialized Project.
    ///
    /// Collection order is the protocol/materialized order. Canonical JSON
    /// normalizes object-key order but deliberately does not turn ordered event
    /// or record collections into sets.
    pub fn from_project(
        project: &Project,
        identity_root: &str,
        dependency_root: &str,
    ) -> Result<Self, String> {
        require_sha256_root("identity_root", identity_root)?;
        require_sha256_root("dependency_root", dependency_root)?;

        Ok(Self {
            schema: SCIENTIFIC_STATE_SCHEMA_V2.to_string(),
            identity_root: identity_root.to_string(),
            dependency_root: dependency_root.to_string(),
            findings_root: scientific_findings_root(&project.findings)?,
            sources_root: collection_root(&project.sources)?,
            evidence_atoms_root: collection_root(&project.evidence_atoms)?,
            condition_records_root: collection_root(&project.condition_records)?,
            review_events_root: collection_root(&project.review_events)?,
            confidence_updates_root: collection_root(&project.confidence_updates)?,
            artifacts_root: collection_root(&project.artifacts)?,
            released_diff_packs_root: collection_root(&project.released_diff_packs)?,
            verdict_conflicts_root: collection_root(&project.verdict_conflicts)?,
            contradictions_root: collection_root(&project.contradictions)?,
            verifier_attachments_root: collection_root(&project.verifier_attachments)?,
            attempts_root: collection_root(&project.attempts)?,
            attempt_resolutions_root: collection_root(&project.attempt_resolutions)?,
            transfers_root: collection_root(&project.transfers)?,
            endorsements_root: collection_root(&project.endorsements)?,
            statement_attestations_root: collection_root(&project.statement_attestations)?,
            anchor_links_root: collection_root(&project.anchor_links)?,
            statement_registrations_root: collection_root(&project.statement_registrations)?,
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCIENTIFIC_STATE_SCHEMA_V2 {
            return Err(format!(
                "scientific state schema must be {SCIENTIFIC_STATE_SCHEMA_V2}"
            ));
        }
        for (field, root) in self.named_roots() {
            require_sha256_root(field, root)?;
        }
        Ok(())
    }

    /// SHA-256 of the canonical closed root record, in `sha256:<64hex>` form.
    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        prefixed_canonical_root(self)
    }

    pub fn verify_root(&self, expected: &str) -> Result<(), String> {
        require_sha256_root("scientific_state_root", expected)?;
        if self.root()? != expected {
            return Err(
                "scientific_state_root does not match the closed component-root record".to_string(),
            );
        }
        Ok(())
    }

    /// Recompute every component from the supplied materialized Project.
    /// Aggregate-root validity alone proves only that this record is internally
    /// addressed; it does not prove that it describes a particular Project.
    pub fn verify_project(&self, project: &Project) -> Result<(), String> {
        let derived = Self::from_project(project, &self.identity_root, &self.dependency_root)?;
        if &derived != self {
            return Err(
                "scientific-state component roots do not match the supplied Project".to_string(),
            );
        }
        Ok(())
    }

    fn named_roots(&self) -> [(&'static str, &str); 20] {
        [
            ("identity_root", &self.identity_root),
            ("dependency_root", &self.dependency_root),
            ("findings_root", &self.findings_root),
            ("sources_root", &self.sources_root),
            ("evidence_atoms_root", &self.evidence_atoms_root),
            ("condition_records_root", &self.condition_records_root),
            ("review_events_root", &self.review_events_root),
            ("confidence_updates_root", &self.confidence_updates_root),
            ("artifacts_root", &self.artifacts_root),
            ("released_diff_packs_root", &self.released_diff_packs_root),
            ("verdict_conflicts_root", &self.verdict_conflicts_root),
            ("contradictions_root", &self.contradictions_root),
            ("verifier_attachments_root", &self.verifier_attachments_root),
            ("attempts_root", &self.attempts_root),
            ("attempt_resolutions_root", &self.attempt_resolutions_root),
            ("transfers_root", &self.transfers_root),
            ("endorsements_root", &self.endorsements_root),
            (
                "statement_attestations_root",
                &self.statement_attestations_root,
            ),
            ("anchor_links_root", &self.anchor_links_root),
            (
                "statement_registrations_root",
                &self.statement_registrations_root,
            ),
        ]
    }
}

pub fn scientific_state_root_v2(
    project: &Project,
    identity_root: &str,
    dependency_root: &str,
) -> Result<String, String> {
    ScientificStateV2::from_project(project, identity_root, dependency_root)?.root()
}

fn collection_root<T: Serialize + ?Sized>(collection: &T) -> Result<String, String> {
    prefixed_canonical_root(collection)
}

fn scientific_findings_root(findings: &[FindingBundle]) -> Result<String, String> {
    let projected = findings
        .iter()
        .map(ScientificFindingV2::from)
        .collect::<Vec<_>>();
    collection_root(&projected)
}

fn prefixed_canonical_root<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    crate::canonical::sha256_canonical(value).map(|digest| format!("sha256:{digest}"))
}

fn require_sha256_root(field: &str, value: &str) -> Result<(), String> {
    if crate::execution_binding::is_full_sha256_root(value) {
        Ok(())
    } else {
        Err(format!(
            "{field} must use the sha256:<64 lowercase hex> form"
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use ed25519_dalek::SigningKey;
    use serde_json::{Value, json};

    use super::*;
    use crate::access_tier::AccessTier;
    use crate::anchor::{Anchor, AnchorKind, AnchorLink, AnchorLinkDraft, JoinPolicy};
    use crate::attempt::{Attempt, AttemptDraft, AttemptResolution, ResolutionEvent};
    use crate::bundle::{Artifact, ConfidenceUpdate, ReviewAction, ReviewEvent};
    use crate::contradiction::Contradiction;
    use crate::endorsement::{Endorsement, EndorsementDraft};
    use crate::events::{StateTarget, compute_event_id};
    use crate::project::{AttemptClaim, StatementRegistration, assemble};
    use crate::released_diff_pack::ReleasedDiffPackRecord;
    use crate::sign::{ActorRecord, SignedEnvelope};
    use crate::sources::{ConditionRecord, EvidenceAtom, SourceRecord};
    use crate::statement_attestation::{
        AttestationDraft, FaithfulnessVerdict, StatementAttestation,
    };
    use crate::test_support::make_finding;
    use crate::transfer::{HomomorphismDescriptor, Transfer, TransferDraft, TransferKind};
    use crate::verdict_conflict::{ConflictDraft, ResolutionMode, VerdictConflict};
    use crate::verifier_attachment::{
        AttachmentDraft, AttachmentOutcome, MatchToClaim, VerifierAttachment, VerifierMethod,
        claim_digest,
    };

    const IDENTITY_ROOT: &str =
        "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    const DEPENDENCY_ROOT: &str =
        "sha256:2222222222222222222222222222222222222222222222222222222222222222";
    const MUTATED_ROOT: &str =
        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

    fn nonempty_project() -> Project {
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let mut project = assemble(
            "state-v2-nonempty",
            vec![make_finding("vf_fixture_a", 0.8, "result")],
            1,
            0,
            "non-empty scientific-state fixture",
        );
        project.frontier_id = Some("vfr_0123456789abcdef".to_string());
        let frontier_id = project.frontier_id();
        let finding_id = project.findings[0].id.clone();

        project.sources.push(SourceRecord {
            id: "vsr_fixture".to_string(),
            source_type: "paper".to_string(),
            locator: "doi:10.0000/fixture".to_string(),
            content_hash: Some(format!("sha256:{}", "33".repeat(32))),
            title: "Fixture source".to_string(),
            authors: vec!["Ada Example".to_string()],
            year: Some(2026),
            doi: Some("10.0000/fixture".to_string()),
            pmid: None,
            imported_at: "2026-07-22T00:00:00Z".to_string(),
            extraction_mode: "fixture".to_string(),
            source_quality: "primary".to_string(),
            caveats: vec!["synthetic conformance record".to_string()],
            finding_ids: vec![finding_id.clone()],
        });
        project.evidence_atoms.push(EvidenceAtom {
            id: "vea_fixture".to_string(),
            source_id: "vsr_fixture".to_string(),
            finding_id: finding_id.clone(),
            locator: Some("page:1".to_string()),
            evidence_type: "exact_recompute".to_string(),
            measurement_or_claim: "fixture evidence".to_string(),
            supports_or_challenges: "supports".to_string(),
            condition_refs: vec!["vcr_fixture".to_string()],
            extraction_method: "fixture".to_string(),
            human_verified: false,
            caveats: vec!["test only".to_string()],
        });
        project.condition_records.push(ConditionRecord {
            id: "vcr_fixture".to_string(),
            finding_id: finding_id.clone(),
            text: "under the frozen fixture".to_string(),
            species: None,
            model_system: "finite model".to_string(),
            method: "enumeration".to_string(),
            exposure_or_efficacy: "not applicable".to_string(),
            comparator_status: "exact".to_string(),
            translation_scope: "fixture only".to_string(),
            caveats: vec!["not scientific evidence".to_string()],
        });
        project.review_events.push(ReviewEvent {
            id: "vreview_fixture".to_string(),
            workspace: None,
            finding_id: finding_id.clone(),
            reviewer: "reviewer:fixture".to_string(),
            reviewed_at: "2026-07-22T00:01:00Z".to_string(),
            scope: Some("fixture".to_string()),
            status: Some("recorded".to_string()),
            action: ReviewAction::Approved,
            reason: "exercise scientific-state mapping".to_string(),
            evidence_considered: vec![],
            state_change: None,
        });
        project.confidence_updates.push(ConfidenceUpdate {
            finding_id: finding_id.clone(),
            previous_score: 0.8,
            new_score: 0.81,
            basis: "fixture update".to_string(),
            updated_by: "agent:fixture".to_string(),
            updated_at: "2026-07-22T00:02:00Z".to_string(),
        });

        let mut artifact = Artifact::new(
            "dataset",
            "fixture artifact",
            format!("sha256:{}", "44".repeat(32)),
            Some(1),
            Some("application/json".to_string()),
            "local_blob",
            Some(".vela/blobs/fixture".to_string()),
            None,
            Some("CC0-1.0".to_string()),
            vec![finding_id.clone()],
            project.findings[0].provenance.clone(),
            BTreeMap::new(),
            AccessTier::Public,
        )
        .expect("build fixture artifact");
        artifact.created = "2026-07-22T00:03:00Z".to_string();
        project.artifacts.push(artifact);

        project
            .released_diff_packs
            .push(ReleasedDiffPackRecord::from_released_event(
                "vsd_fixture".to_string(),
                frontier_id.clone(),
                "fixture pack".to_string(),
                "scientific_state".to_string(),
                "2026-07-22T00:04:00Z".to_string(),
                "vev_fixture_pack".to_string(),
                vec!["vpr_fixture".to_string()],
            ));
        project.verdict_conflicts.push(
            VerdictConflict::build(ConflictDraft {
                frontier_id: frontier_id.clone(),
                verdicts: vec![
                    "vpv_aaaaaaaaaaaaaaaa".to_string(),
                    "vpv_bbbbbbbbbbbbbbbb".to_string(),
                ],
                shared_member_ids: vec!["vpr_aaaaaaaaaaaaaaaa".to_string()],
                resolution_mode: ResolutionMode::Escalation,
                resolution_actor: "reviewer:fixture".to_string(),
                resolved_at: "2026-07-22T00:05:00Z".to_string(),
                winning_verdict_id: None,
                rationale: Some("fixture disagreement".to_string()),
            })
            .expect("build fixture verdict conflict"),
        );
        project.contradictions.push(Contradiction::candidate(
            &frontier_id,
            &finding_id,
            "vf_fixture_b",
            "fixture contradiction",
        ));

        let attachment = VerifierAttachment::build(AttachmentDraft {
            target: finding_id.clone(),
            claim_digest: claim_digest("Finding vf_fixture_a"),
            verifier_method: VerifierMethod::ExactArithmeticRecompute,
            solver_id: "fixture-solver".to_string(),
            independent_of: vec![],
            match_to_claim: MatchToClaim {
                matches: true,
                checker_actor: "agent:fixture".to_string(),
            },
            adversarial_probes: vec![],
            outcome: AttachmentOutcome::Passed,
            verifier_actor: "agent:fixture".to_string(),
            note: "fixture attachment".to_string(),
        })
        .expect("build fixture verifier attachment");
        project.verifier_attachments.push(attachment);

        let attempt = Attempt::build(
            AttemptDraft {
                problem: 1,
                kind: "bounded_search".to_string(),
                claim: "fixture attempt".to_string(),
                ..AttemptDraft::default()
            },
            &key,
        )
        .expect("build fixture attempt");
        let attempt_id = attempt.attempt_id.clone();
        project.attempts.push(attempt);
        project.attempt_resolutions.push(
            ResolutionEvent::new(
                &attempt_id,
                AttemptResolution::Refuted {
                    by_probe: "fixture-probe".to_string(),
                },
                "agent:fixture",
                "2026-07-22T00:06:00Z",
                "fixture resolution",
            )
            .expect("build fixture resolution"),
        );
        project.transfers.push(
            Transfer::build(
                TransferDraft {
                    source_claim: finding_id.clone(),
                    source_claim_digest: claim_digest("Finding vf_fixture_a"),
                    source_gate_status_claimed: "needs_verification".to_string(),
                    source_attachments: vec![],
                    target_claim: "vf_fixture_b".to_string(),
                    target_premise_digest: claim_digest("fixture premise"),
                    target_attachments: vec![],
                    homomorphism: HomomorphismDescriptor {
                        kind: TransferKind::FrozenVerifier,
                        map_decl: "fixture-verifier".to_string(),
                        source_type: "fixture-a".to_string(),
                        target_type: "fixture-b".to_string(),
                        theorem_verification: String::new(),
                        theorem_id: None,
                    },
                    provenance: Default::default(),
                    note: "fixture transfer".to_string(),
                },
                &key,
            )
            .expect("build fixture transfer"),
        );
        project.endorsements.push(
            Endorsement::build(
                EndorsementDraft {
                    target_record: finding_id.clone(),
                    endorser: "reviewer:fixture".to_string(),
                    dimension: "useful".to_string(),
                    rationale: "fixture endorsement".to_string(),
                    at: "2026-07-22T00:07:00Z".to_string(),
                },
                &key,
            )
            .expect("build fixture endorsement"),
        );
        project.statement_attestations.push(
            StatementAttestation::build(
                AttestationDraft {
                    target: finding_id.clone(),
                    informal_ref: "fixture:1".to_string(),
                    formal_ref: "Fixture.lean@deadbeef".to_string(),
                    formal_statement_hash: "55".repeat(32),
                    verdict: FaithfulnessVerdict::Variant,
                    note: "fixture statement comparison".to_string(),
                    attested_by: "reviewer:fixture".to_string(),
                    attested_at: "2026-07-22T00:08:00Z".to_string(),
                },
                &key,
            )
            .expect("build fixture statement attestation"),
        );
        project.anchor_links.push(
            AnchorLink::build(
                AnchorLinkDraft {
                    target: finding_id.clone(),
                    anchor: Anchor {
                        namespace: "fixture".to_string(),
                        id: "1".to_string(),
                        role: "exact statement".to_string(),
                        kind: AnchorKind::Statement,
                        join_policy: JoinPolicy::HardIdentity,
                        namespace_version: Some("v1".to_string()),
                        source_revision: Some("deadbeef".to_string()),
                        statement_fingerprint: Some("66".repeat(32)),
                    },
                    attached_by: "reviewer:fixture".to_string(),
                    attached_at: "2026-07-22T00:09:00Z".to_string(),
                },
                &key,
            )
            .expect("build fixture anchor link"),
        );
        project.statement_registrations.push(StatementRegistration {
            statement_hash: "77".repeat(32),
            informal_ref: "fixture:1".to_string(),
            registered_by: "agent:fixture".to_string(),
            registered_at: "2026-07-22T00:10:00Z".to_string(),
            finding_id: Some(finding_id),
        });

        project
    }

    #[test]
    fn scientific_state_root_v2_empty_vector_is_pinned() {
        let project = assemble("state-v2", vec![], 0, 0, "fixture");
        let state = ScientificStateV2::from_project(&project, IDENTITY_ROOT, DEPENDENCY_ROOT)
            .expect("derive state");
        state.verify_project(&project).unwrap();
        let empty_root = collection_root(&Vec::<serde_json::Value>::new()).unwrap();
        assert_eq!(
            empty_root,
            "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
        );

        for (field, root) in state.named_roots() {
            if matches!(field, "identity_root" | "dependency_root") {
                continue;
            }
            assert_eq!(root, empty_root, "{field} must bind canonical []");
        }
        assert_eq!(
            state.root().unwrap(),
            "sha256:eb494f16c85a588b595f8b7209099a36e9429f0804824336fcbb5ae963fbdf3c",
            "the empty-state vector is a cross-implementation contract"
        );
    }

    #[test]
    fn scientific_state_root_v2_maps_every_nonempty_project_collection() {
        let project = nonempty_project();
        let state = ScientificStateV2::from_project(&project, IDENTITY_ROOT, DEPENDENCY_ROOT)
            .expect("derive non-empty state");

        macro_rules! mapped_root {
            ($root_field:ident, $project_field:ident) => {
                assert_eq!(
                    state.$root_field,
                    collection_root(&project.$project_field).unwrap(),
                    concat!(
                        stringify!($root_field),
                        " must map Project.",
                        stringify!($project_field)
                    )
                );
            };
        }

        assert_eq!(
            state.findings_root,
            scientific_findings_root(&project.findings).unwrap(),
            "findings_root must map the closed scientific finding projection"
        );
        mapped_root!(sources_root, sources);
        mapped_root!(evidence_atoms_root, evidence_atoms);
        mapped_root!(condition_records_root, condition_records);
        mapped_root!(review_events_root, review_events);
        mapped_root!(confidence_updates_root, confidence_updates);
        mapped_root!(artifacts_root, artifacts);
        mapped_root!(released_diff_packs_root, released_diff_packs);
        mapped_root!(verdict_conflicts_root, verdict_conflicts);
        mapped_root!(contradictions_root, contradictions);
        mapped_root!(verifier_attachments_root, verifier_attachments);
        mapped_root!(attempts_root, attempts);
        mapped_root!(attempt_resolutions_root, attempt_resolutions);
        mapped_root!(transfers_root, transfers);
        mapped_root!(endorsements_root, endorsements);
        mapped_root!(statement_attestations_root, statement_attestations);
        mapped_root!(anchor_links_root, anchor_links);
        mapped_root!(statement_registrations_root, statement_registrations);

        let empty_root = collection_root(&Vec::<Value>::new()).unwrap();
        let collection_roots: BTreeSet<_> = state
            .named_roots()
            .into_iter()
            .filter(|(field, _)| !matches!(*field, "identity_root" | "dependency_root"))
            .map(|(_, root)| root)
            .collect();
        assert_eq!(
            collection_roots.len(),
            18,
            "the fixture keeps every collection root distinguishable so swapped mappings fail"
        );
        assert!(
            collection_roots.iter().all(|root| **root != empty_root),
            "every scientific collection in the mapping fixture must be non-empty"
        );
        state.verify_project(&project).unwrap();
    }

    #[test]
    fn scientific_state_root_v2_canonicalizes_object_keys_but_preserves_array_order() {
        let original: Value =
            serde_json::from_str(r#"[{"z":3,"a":1,"m":2},{"id":"second","value":4}]"#).unwrap();
        let reordered_keys: Value =
            serde_json::from_str(r#"[{"m":2,"z":3,"a":1},{"value":4,"id":"second"}]"#).unwrap();
        let reversed_records: Value =
            serde_json::from_str(r#"[{"id":"second","value":4},{"a":1,"m":2,"z":3}]"#).unwrap();

        assert_eq!(
            collection_root(&original).unwrap(),
            collection_root(&reordered_keys).unwrap(),
            "canonical JSON object-key order is not semantic"
        );
        assert_ne!(
            collection_root(&original).unwrap(),
            collection_root(&reversed_records).unwrap(),
            "materialized collection order remains part of the protocol preimage"
        );
    }

    #[test]
    fn scientific_state_root_v2_nonempty_vector_is_pinned() {
        let project = nonempty_project();
        let state = ScientificStateV2::from_project(&project, IDENTITY_ROOT, DEPENDENCY_ROOT)
            .expect("derive non-empty state");
        let expected = ScientificStateV2 {
            schema: SCIENTIFIC_STATE_SCHEMA_V2.to_string(),
            identity_root: IDENTITY_ROOT.to_string(),
            dependency_root: DEPENDENCY_ROOT.to_string(),
            findings_root:
                "sha256:70b046c47d39aa807b2b15bb1afb44281b6fa1ee662bc923c1ecf5c9a05be15f"
                    .to_string(),
            sources_root: "sha256:c0795b40bc3bf96765c537bb36a2959edd00374b548cf1977065edb508c6e064"
                .to_string(),
            evidence_atoms_root:
                "sha256:4b180fb98dbc0f1e745d244f6aa11efe732116346a71eb3439c6f7fd367d2aad"
                    .to_string(),
            condition_records_root:
                "sha256:722cfad7dfc7bf582f8c43f7c5b34d3b7888cc4119d1e705a49b0a70acce5cc5"
                    .to_string(),
            review_events_root:
                "sha256:3b0f757e21d4ff18997f2f3f8275fc82ad72dd1e180ea95b2a294c5e01e5cc20"
                    .to_string(),
            confidence_updates_root:
                "sha256:80a432192d48a2fe50c5a7a622b201494bcf2f77c742a4a70ecc36f5112a461f"
                    .to_string(),
            artifacts_root:
                "sha256:6a53681e9f093b80c5bee9b1a48d69b53dbba4f0e24361951aea162a3952099d"
                    .to_string(),
            released_diff_packs_root:
                "sha256:17937c13a11900e5c3c4c8eeadd7347a8c091b27ae899b8d6607fa212a32ed3e"
                    .to_string(),
            verdict_conflicts_root:
                "sha256:592884b6313dbaea819dfee77de7fdb48a59ddb9946ac5d34ace4dabcc69e83f"
                    .to_string(),
            contradictions_root:
                "sha256:aa637676d044b75a4b7f5d69b8e8fed2fc5d357fc7a87824ec0b1cd6cec9ea8e"
                    .to_string(),
            verifier_attachments_root:
                "sha256:168d11b7ba6701d6eeab46614739be4b303332c9dd743328639082d0349644ba"
                    .to_string(),
            attempts_root:
                "sha256:f6df7caa173472940c8b509f945683d54ae7eba3611b9341437ee800ebeb53fa"
                    .to_string(),
            attempt_resolutions_root:
                "sha256:8102a47ae24aaea7b75dfb6142d20eda133003b0252b4cfb0c23b731084457a3"
                    .to_string(),
            transfers_root:
                "sha256:41ac416a4d0bb7d34405747543e4936cd9f1631b2142910d1f78072c0df0758d"
                    .to_string(),
            endorsements_root:
                "sha256:005ba20b9177d71fc84f2afe474345d9dc42e46344a03e14f34e0809a3666608"
                    .to_string(),
            statement_attestations_root:
                "sha256:a2fb219146e44a2314031cf94a8a13c74b06703f596c3470b2e44160413b42e6"
                    .to_string(),
            anchor_links_root:
                "sha256:6160cc0ea07d8f7684e2be6d1c81fd214314c6fe72db5268e520d60c8d4930e4"
                    .to_string(),
            statement_registrations_root:
                "sha256:5c00043b15a18e22223875252335f8b0f10937ab58367463c36818a4331127e2"
                    .to_string(),
        };
        assert_eq!(state, expected, "pin every non-empty component root");
        assert_eq!(
            state.root().unwrap(),
            "sha256:f864d87f058b877d67c4ddbfb18619e0b22e43c5089419acee39072413464eaa",
            "the non-empty state vector is a cross-implementation contract"
        );
    }

    #[test]
    fn scientific_state_root_v2_binds_every_named_component_root() {
        let project = assemble("state-v2", vec![], 0, 0, "fixture");
        let state = ScientificStateV2::from_project(&project, IDENTITY_ROOT, DEPENDENCY_ROOT)
            .expect("derive state");
        let baseline = state.root().unwrap();

        macro_rules! changes_root {
            ($field:ident) => {{
                let mut changed = state.clone();
                changed.$field = MUTATED_ROOT.to_string();
                assert_ne!(changed.root().unwrap(), baseline, stringify!($field));
            }};
        }

        changes_root!(identity_root);
        changes_root!(dependency_root);
        changes_root!(findings_root);
        changes_root!(sources_root);
        changes_root!(evidence_atoms_root);
        changes_root!(condition_records_root);
        changes_root!(review_events_root);
        changes_root!(confidence_updates_root);
        changes_root!(artifacts_root);
        changes_root!(released_diff_packs_root);
        changes_root!(verdict_conflicts_root);
        changes_root!(contradictions_root);
        changes_root!(verifier_attachments_root);
        changes_root!(attempts_root);
        changes_root!(attempt_resolutions_root);
        changes_root!(transfers_root);
        changes_root!(endorsements_root);
        changes_root!(statement_attestations_root);
        changes_root!(anchor_links_root);
        changes_root!(statement_registrations_root);
    }

    #[test]
    fn scientific_state_root_v2_excludes_display_operational_metadata() {
        let mut project = assemble("state-v2", vec![], 0, 0, "fixture");
        let baseline = scientific_state_root_v2(&project, IDENTITY_ROOT, DEPENDENCY_ROOT).unwrap();

        project.project.name = "renamed display title".to_string();
        project.project.description = "changed scope prose".to_string();
        project.project.compiled_at = "2099-01-01T00:00:00Z".to_string();
        project.project.compiler = "display-only compiler".to_string();
        project.stats.findings = 99;
        assert_eq!(
            scientific_state_root_v2(&project, IDENTITY_ROOT, DEPENDENCY_ROOT).unwrap(),
            baseline
        );

        let mut event = project.events[0].clone();
        event.reason = "excluded event mutation".to_string();
        event.id = compute_event_id(&event);
        project.events.push(event);
        project.proposals.push(crate::proposals::new_proposal(
            "finding.add",
            StateTarget {
                r#type: "frontier".to_string(),
                id: project.frontier_id(),
            },
            "agent:fixture",
            "agent",
            "excluded pending proposal",
            json!({}),
            vec![],
            vec![],
        ));
        project.signatures.push(SignedEnvelope {
            finding_id: "vf_0000000000000000".to_string(),
            signature: "00".repeat(64),
            public_key: "00".repeat(32),
            signed_at: "2026-07-22T00:00:00Z".to_string(),
            algorithm: "ed25519".to_string(),
        });
        project.actors.push(ActorRecord {
            id: "reviewer:fixture".to_string(),
            public_key: "00".repeat(32),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        project.proof_state.stale_reason = Some("excluded proof metadata".to_string());
        project.attempt_claims.push(AttemptClaim {
            obligation_id: "obligation:fixture".to_string(),
            claimant_actor: "agent:fixture".to_string(),
            claimant_pubkey: "00".repeat(32),
            claimed_at: "2026-07-22T00:00:00Z".to_string(),
            lease_ttl_seconds: 60,
            claim_event_id: None,
        });

        assert_eq!(
            scientific_state_root_v2(&project, IDENTITY_ROOT, DEPENDENCY_ROOT).unwrap(),
            baseline,
            "events, proposals, signatures, actors, proof state, and leases are not scientific state"
        );
    }

    #[test]
    fn scientific_state_root_v2_excludes_finding_links_and_access_tier() {
        let mut project = nonempty_project();
        let baseline = scientific_state_root_v2(&project, IDENTITY_ROOT, DEPENDENCY_ROOT).unwrap();

        project.findings[0].links.push(crate::bundle::Link {
            target: "vf_graph_only".to_string(),
            link_type: "supports".to_string(),
            note: "mutable review surface".to_string(),
            inferred_by: "fixture".to_string(),
            created_at: "2026-07-22T01:00:00Z".to_string(),
            mechanism: None,
        });
        project.findings[0].access_tier = AccessTier::Restricted;
        assert_eq!(
            scientific_state_root_v2(&project, IDENTITY_ROOT, DEPENDENCY_ROOT).unwrap(),
            baseline,
            "graph links and read-side disclosure policy are not scientific content"
        );

        project.findings[0].assertion.text =
            "changed scientific assertion must change the root".to_string();
        assert_ne!(
            scientific_state_root_v2(&project, IDENTITY_ROOT, DEPENDENCY_ROOT).unwrap(),
            baseline
        );
    }

    #[test]
    fn scientific_state_root_v2_rejects_invalid_unknown_or_mismatched_records() {
        let project = assemble("state-v2", vec![], 0, 0, "fixture");
        assert!(
            ScientificStateV2::from_project(&project, "sha256:short", DEPENDENCY_ROOT).is_err()
        );

        let state = ScientificStateV2::from_project(&project, IDENTITY_ROOT, DEPENDENCY_ROOT)
            .expect("derive state");
        let mut value = serde_json::to_value(&state).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("future_field".to_string(), json!(MUTATED_ROOT));
        assert!(serde_json::from_value::<ScientificStateV2>(value).is_err());
        assert!(state.verify_root(MUTATED_ROOT).is_err());

        let mut mismatched = state.clone();
        mismatched.findings_root = MUTATED_ROOT.to_string();
        assert!(mismatched.verify_project(&project).is_err());
    }
}
