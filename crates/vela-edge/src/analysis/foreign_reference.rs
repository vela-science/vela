//! Derived, non-authoritative reference to exact state accepted by another
//! Frontier.
//!
//! The envelope is a portable manifest over retained source objects. It does
//! not import source authority, change local Standing, or require a hosted
//! resolver. A receiving Frontier may retain the bytes as evidence and apply
//! its own ordinary Submission, Verification, and Decision rules.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use vela_protocol::authority::{
    AuthorityEnvelopeV1, AuthorityEventV1, AuthorityKeysetV1, AuthorityRecordV1,
    verify_authority_envelope,
};
use vela_protocol::claim_record::ClaimRecordV1;
use vela_protocol::current_repository::CurrentRepositoryV3;
use vela_protocol::events::EventKind;
use vela_protocol::proposal_v1::ProposalV1;
use vela_protocol::repository_origin::{RepositoryOriginKind, RepositoryOriginV1};
use vela_protocol::submission_v1::SubmissionV1;
use vela_protocol::verification_record::VerificationRecordV1;

pub const FOREIGN_REFERENCE_SCHEMA_V1: &str = "vela.foreign-reference.v1";
pub const FOREIGN_REFERENCE_ASSESSMENT_SCHEMA_V1: &str = "vela.foreign-reference-assessment.v1";

const REQUIRED_ROLES: [&str; 11] = [
    "applied_event",
    "authority_keyset",
    "authority_record",
    "claim",
    "current_repository_manifest",
    "decision_event",
    "proposal",
    "repository_origin",
    "submission",
    "transition_repository_manifest",
    "verification",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactObjectRef {
    pub id: String,
    pub root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForeignRepositoryRefV1 {
    pub git_commit: String,
    pub git_tree: String,
    pub repository_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticEventRefV1 {
    pub id: String,
    pub root: String,
    pub semantic_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForeignSourceV1 {
    pub frontier_id: String,
    pub current_repository: ForeignRepositoryRefV1,
    pub transition_repository: ForeignRepositoryRefV1,
    pub repository_origin: ExactObjectRef,
    pub claim: ExactObjectRef,
    pub submission: ExactObjectRef,
    pub proposal: ExactObjectRef,
    pub verification: ExactObjectRef,
    pub decision_event: ExactObjectRef,
    pub applied_event: SemanticEventRefV1,
    pub authority_record: ExactObjectRef,
    pub authority_keyset_root: String,
    pub standing: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForeignObjectRefV1 {
    pub role: String,
    pub id: String,
    pub root: String,
    pub bytes_root: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForeignCompletenessV1 {
    pub status: String,
    pub missing_roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForeignAuthorityBoundaryV1 {
    pub source_standing: String,
    pub local_standing_effect: String,
    pub requires_local_decision: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForeignReferenceV1 {
    pub schema: String,
    pub source: ForeignSourceV1,
    pub objects: Vec<ForeignObjectRefV1>,
    pub object_set_root: String,
    pub completeness: ForeignCompletenessV1,
    pub authority: ForeignAuthorityBoundaryV1,
    pub does_not_establish: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForeignReferenceAssessmentV1 {
    pub schema: String,
    pub status: String,
    pub reference_root: String,
    pub object_set_root: String,
    pub source_frontier_id: String,
    pub source_current_git_commit: String,
    pub source_current_git_tree: String,
    pub source_current_repository_root: String,
    pub source_transition_git_commit: String,
    pub source_transition_git_tree: String,
    pub source_transition_repository_root: String,
    pub source_repository_origin_id: String,
    pub source_repository_origin_root: String,
    pub source_claim_id: String,
    pub source_claim_root: String,
    pub source_submission_id: String,
    pub source_submission_root: String,
    pub source_proposal_id: String,
    pub source_proposal_root: String,
    pub source_verification_id: String,
    pub source_verification_root: String,
    pub source_decision_event_id: String,
    pub source_decision_event_root: String,
    pub source_applied_event_id: String,
    pub source_applied_event_root: String,
    pub source_applied_semantic_event_id: String,
    pub source_authority_record_id: String,
    pub source_authority_record_root: String,
    pub source_authority_keyset_root: String,
    pub source_standing: String,
    pub local_standing_effect: String,
    pub requires_local_decision: bool,
    pub diagnostics: Vec<String>,
}

pub fn foreign_reference_root(reference: &ForeignReferenceV1) -> Result<String, String> {
    canonical_root(reference)
}

pub fn foreign_object_set_root(objects: &[ForeignObjectRefV1]) -> Result<String, String> {
    canonical_root(&objects)
}

pub fn assess_foreign_reference(
    reference: &ForeignReferenceV1,
) -> Result<ForeignReferenceAssessmentV1, String> {
    validate_reference(reference)?;
    let diagnostics = reference
        .completeness
        .missing_roles
        .iter()
        .map(|role| format!("missing_role:{role}"))
        .collect::<Vec<_>>();

    Ok(ForeignReferenceAssessmentV1 {
        schema: FOREIGN_REFERENCE_ASSESSMENT_SCHEMA_V1.to_string(),
        status: reference.completeness.status.clone(),
        reference_root: foreign_reference_root(reference)?,
        object_set_root: reference.object_set_root.clone(),
        source_frontier_id: reference.source.frontier_id.clone(),
        source_current_git_commit: reference.source.current_repository.git_commit.clone(),
        source_current_git_tree: reference.source.current_repository.git_tree.clone(),
        source_current_repository_root: reference.source.current_repository.repository_root.clone(),
        source_transition_git_commit: reference.source.transition_repository.git_commit.clone(),
        source_transition_git_tree: reference.source.transition_repository.git_tree.clone(),
        source_transition_repository_root: reference
            .source
            .transition_repository
            .repository_root
            .clone(),
        source_repository_origin_id: reference.source.repository_origin.id.clone(),
        source_repository_origin_root: reference.source.repository_origin.root.clone(),
        source_claim_id: reference.source.claim.id.clone(),
        source_claim_root: reference.source.claim.root.clone(),
        source_submission_id: reference.source.submission.id.clone(),
        source_submission_root: reference.source.submission.root.clone(),
        source_proposal_id: reference.source.proposal.id.clone(),
        source_proposal_root: reference.source.proposal.root.clone(),
        source_verification_id: reference.source.verification.id.clone(),
        source_verification_root: reference.source.verification.root.clone(),
        source_decision_event_id: reference.source.decision_event.id.clone(),
        source_decision_event_root: reference.source.decision_event.root.clone(),
        source_applied_event_id: reference.source.applied_event.id.clone(),
        source_applied_event_root: reference.source.applied_event.root.clone(),
        source_applied_semantic_event_id: reference.source.applied_event.semantic_id.clone(),
        source_authority_record_id: reference.source.authority_record.id.clone(),
        source_authority_record_root: reference.source.authority_record.root.clone(),
        source_authority_keyset_root: reference.source.authority_keyset_root.clone(),
        source_standing: reference.source.standing.clone(),
        local_standing_effect: reference.authority.local_standing_effect.clone(),
        requires_local_decision: reference.authority.requires_local_decision,
        diagnostics,
    })
}

pub fn verify_foreign_reference_package(
    reference: &ForeignReferenceV1,
    package_root: &Path,
) -> Result<ForeignReferenceAssessmentV1, String> {
    let assessment = assess_foreign_reference(reference)?;
    let canonical_package_root = package_root
        .canonicalize()
        .map_err(|_| "foreign_reference_package_unavailable".to_string())?;
    let mut object_bytes = BTreeMap::new();
    for object in &reference.objects {
        let object_path = package_root.join(&object.path);
        let canonical_object_path = object_path
            .canonicalize()
            .map_err(|_| format!("foreign_reference_object_unavailable:{}", object.role))?;
        if !canonical_object_path.starts_with(&canonical_package_root) {
            return Err(format!(
                "foreign_reference_object_path_escape:{}",
                object.role
            ));
        }
        let bytes = fs::read(canonical_object_path)
            .map_err(|_| format!("foreign_reference_object_unavailable:{}", object.role))?;
        let observed = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
        if observed != object.bytes_root {
            return Err(format!(
                "foreign_reference_object_bytes_mismatch:{}",
                object.role
            ));
        }
        object_bytes.insert(object.role.as_str(), bytes);
    }
    if reference.completeness.status == "complete" {
        validate_semantic_package(reference, &object_bytes)?;
    }
    Ok(assessment)
}

fn validate_reference(reference: &ForeignReferenceV1) -> Result<(), String> {
    if reference.schema != FOREIGN_REFERENCE_SCHEMA_V1 {
        return Err("foreign_reference_schema_invalid".to_string());
    }
    require_prefixed_hex(&reference.source.frontier_id, "vfr_", 16, "frontier_id")?;
    require_repository_ref(&reference.source.current_repository, "current_repository")?;
    require_repository_ref(
        &reference.source.transition_repository,
        "transition_repository",
    )?;
    require_exact_ref(&reference.source.repository_origin, "vro_", 16)?;
    require_exact_ref(&reference.source.claim, "vcl_", 64)?;
    require_exact_ref(&reference.source.submission, "vsb_", 16)?;
    require_exact_ref(&reference.source.proposal, "vpr_", 16)?;
    require_exact_ref(&reference.source.verification, "vvr_", 16)?;
    require_exact_ref(&reference.source.decision_event, "vev_", 16)?;
    require_prefixed_hex(&reference.source.applied_event.id, "vev_", 16, "object_id")?;
    require_sha256(&reference.source.applied_event.root)?;
    require_prefixed_hex(
        &reference.source.applied_event.semantic_id,
        "vev_",
        16,
        "semantic_event_id",
    )?;
    require_exact_ref(&reference.source.authority_record, "var_", 16)?;
    require_sha256(&reference.source.authority_keyset_root)?;
    if reference.source.standing != "accepted" {
        return Err("foreign_reference_source_standing_invalid".to_string());
    }
    if reference.authority.source_standing != reference.source.standing {
        return Err("foreign_reference_source_standing_mismatch".to_string());
    }
    if reference.authority.local_standing_effect != "none"
        || !reference.authority.requires_local_decision
    {
        return Err("foreign_reference_authority_escalation".to_string());
    }
    if reference.does_not_establish.is_empty() {
        return Err("foreign_reference_nonclaims_missing".to_string());
    }
    let mut nonclaims = BTreeSet::new();
    for nonclaim in &reference.does_not_establish {
        require_text(nonclaim)?;
        if !nonclaims.insert(nonclaim.as_str()) {
            return Err("foreign_reference_nonclaim_duplicate".to_string());
        }
    }

    let mut sorted = reference.objects.clone();
    sorted.sort();
    if sorted != reference.objects {
        return Err("foreign_reference_objects_unsorted".to_string());
    }
    let mut roles = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut roots = BTreeSet::new();
    let mut bytes_roots = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for object in &reference.objects {
        require_text(&object.role)?;
        require_text(&object.id)?;
        require_sha256(&object.root)?;
        require_sha256(&object.bytes_root)?;
        require_relative_path(&object.path)?;
        if !roles.insert(object.role.as_str()) {
            return Err("foreign_reference_role_duplicate".to_string());
        }
        if !ids.insert(object.id.as_str()) {
            return Err("foreign_reference_object_id_duplicate".to_string());
        }
        if !roots.insert(object.root.as_str()) {
            return Err("foreign_reference_object_root_duplicate".to_string());
        }
        if !bytes_roots.insert(object.bytes_root.as_str()) {
            return Err("foreign_reference_object_bytes_root_duplicate".to_string());
        }
        if !paths.insert(object.path.as_str()) {
            return Err("foreign_reference_object_path_duplicate".to_string());
        }
    }
    if foreign_object_set_root(&reference.objects)? != reference.object_set_root {
        return Err("foreign_reference_object_set_root_mismatch".to_string());
    }

    let missing = reference
        .completeness
        .missing_roles
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if missing.len() != reference.completeness.missing_roles.len()
        || !reference
            .completeness
            .missing_roles
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        || missing.iter().any(|role| !REQUIRED_ROLES.contains(role))
    {
        return Err("foreign_reference_missing_roles_invalid".to_string());
    }
    let actually_missing = REQUIRED_ROLES
        .iter()
        .copied()
        .filter(|role| !roles.contains(role))
        .collect::<BTreeSet<_>>();
    if actually_missing != missing {
        return Err("foreign_reference_completeness_mismatch".to_string());
    }
    match reference.completeness.status.as_str() {
        "complete" if missing.is_empty() => {}
        "incomplete" if !missing.is_empty() => {}
        _ => return Err("foreign_reference_completeness_invalid".to_string()),
    }

    require_role_binding(
        reference,
        "current_repository_manifest",
        "",
        None,
        &reference.source.current_repository.repository_root,
    )?;
    require_role_binding(
        reference,
        "transition_repository_manifest",
        "",
        None,
        &reference.source.transition_repository.repository_root,
    )?;
    require_role_binding(
        reference,
        "repository_origin",
        "",
        Some(&reference.source.repository_origin.id),
        &reference.source.repository_origin.root,
    )?;
    require_role_binding(
        reference,
        "authority_keyset",
        "",
        None,
        &reference.source.authority_keyset_root,
    )?;
    require_role_binding(
        reference,
        "claim",
        "",
        Some(&reference.source.claim.id),
        &reference.source.claim.root,
    )?;
    require_role_binding(
        reference,
        "submission",
        "",
        Some(&reference.source.submission.id),
        &reference.source.submission.root,
    )?;
    require_role_binding(
        reference,
        "proposal",
        "",
        Some(&reference.source.proposal.id),
        &reference.source.proposal.root,
    )?;
    require_role_binding(
        reference,
        "verification",
        "",
        Some(&reference.source.verification.id),
        &reference.source.verification.root,
    )?;
    require_role_binding(
        reference,
        "decision_event",
        "",
        Some(&reference.source.decision_event.id),
        &reference.source.decision_event.root,
    )?;
    require_role_binding(
        reference,
        "applied_event",
        "",
        Some(&reference.source.applied_event.id),
        &reference.source.applied_event.root,
    )?;
    require_role_binding(
        reference,
        "authority_record",
        "",
        Some(&reference.source.authority_record.id),
        &reference.source.authority_record.root,
    )?;

    Ok(())
}

fn require_repository_ref(reference: &ForeignRepositoryRefV1, field: &str) -> Result<(), String> {
    require_hex(&reference.git_commit, 40, &format!("{field}_git_commit"))?;
    require_hex(&reference.git_tree, 40, &format!("{field}_git_tree"))?;
    require_sha256(&reference.repository_root)
}

fn validate_semantic_package(
    reference: &ForeignReferenceV1,
    objects: &BTreeMap<&str, Vec<u8>>,
) -> Result<(), String> {
    let bytes = |role: &str| {
        objects
            .get(role)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("foreign_reference_required_role_missing:{role}"))
    };

    let current = CurrentRepositoryV3::parse(bytes("current_repository_manifest")?)
        .map_err(|_| "foreign_reference_current_repository_invalid".to_string())?;
    if current.frontier_id != reference.source.frontier_id
        || current.canonical_root()? != reference.source.current_repository.repository_root
        || current.origin_id != reference.source.repository_origin.id
        || current.origin_root != reference.source.repository_origin.root
        || !current.accepted_claims.iter().any(|claim| {
            claim.claim_id == reference.source.claim.id
                && claim.claim_root == reference.source.claim.root
                && claim.standing == "accepted"
        })
    {
        return Err("foreign_reference_current_repository_mismatch".to_string());
    }

    let origin = RepositoryOriginV1::parse(bytes("repository_origin")?)
        .map_err(|_| "foreign_reference_repository_origin_invalid".to_string())?;
    let predecessor = origin
        .predecessor
        .as_ref()
        .ok_or_else(|| "foreign_reference_repository_origin_not_compaction".to_string())?;
    if origin.kind != RepositoryOriginKind::Compaction
        || origin.origin_id != reference.source.repository_origin.id
        || origin.canonical_root()? != reference.source.repository_origin.root
        || origin.frontier_id != reference.source.frontier_id
        || predecessor.commit != reference.source.transition_repository.git_commit
        || predecessor.tree != reference.source.transition_repository.git_tree
        || predecessor.repository_root != reference.source.transition_repository.repository_root
        || predecessor.authority_head_root != reference.source.authority_record.root
    {
        return Err("foreign_reference_repository_origin_mismatch".to_string());
    }

    let transition_bytes = bytes("transition_repository_manifest")?;
    require_canonical_json(
        transition_bytes,
        "foreign_reference_transition_repository_not_canonical",
    )?;
    let transition: Value = serde_json::from_slice(transition_bytes)
        .map_err(|_| "foreign_reference_transition_repository_invalid".to_string())?;
    if transition.get("schema").and_then(Value::as_str) != Some("vela.repository.v2")
        || transition.get("frontier_id").and_then(Value::as_str)
            != Some(reference.source.frontier_id.as_str())
        || transition
            .get("authority_keyset_root")
            .and_then(Value::as_str)
            != Some(reference.source.authority_keyset_root.as_str())
    {
        return Err("foreign_reference_transition_repository_mismatch".to_string());
    }
    require_transition_claim(&transition, reference)?;
    require_transition_object(&transition, "submissions", &reference.source.submission)?;
    require_transition_object(&transition, "proposals", &reference.source.proposal)?;
    require_transition_object(&transition, "verifications", &reference.source.verification)?;

    let claim = ClaimRecordV1::parse(bytes("claim")?)
        .map_err(|_| "foreign_reference_claim_invalid".to_string())?;
    if claim.claim_id != reference.source.claim.id
        || claim.canonical_root()? != reference.source.claim.root
    {
        return Err("foreign_reference_claim_mismatch".to_string());
    }
    let submission = SubmissionV1::parse(bytes("submission")?)
        .map_err(|_| "foreign_reference_submission_invalid".to_string())?;
    if submission.submission_id != reference.source.submission.id
        || submission.canonical_root()? != reference.source.submission.root
        || submission.claim.assertion != claim.assertion.text
        || submission.claim.claim_type != claim.assertion.kind
    {
        return Err("foreign_reference_submission_mismatch".to_string());
    }
    let proposal = ProposalV1::parse(bytes("proposal")?)
        .map_err(|_| "foreign_reference_proposal_invalid".to_string())?;
    if proposal.proposal_id != reference.source.proposal.id
        || proposal.canonical_root()? != reference.source.proposal.root
        || proposal.action != "claim.revise"
        || proposal.subject.id != reference.source.claim.id
        || proposal.subject.root != reference.source.claim.root
        || proposal.producer_package.id != reference.source.submission.id
        || proposal.producer_package.root != reference.source.submission.root
    {
        return Err("foreign_reference_proposal_mismatch".to_string());
    }
    let verification = VerificationRecordV1::parse(bytes("verification")?)
        .map_err(|_| "foreign_reference_verification_invalid".to_string())?;
    if verification.verification_record_id != reference.source.verification.id
        || verification.canonical_root()? != reference.source.verification.root
        || verification.outcome != "pass"
        || verification.subject.claim_id != reference.source.claim.id
        || verification.subject.submission_id != reference.source.submission.id
        || verification.subject.submission_root != reference.source.submission.root
        || verification.subject.proposal_id != reference.source.proposal.id
    {
        return Err("foreign_reference_verification_mismatch".to_string());
    }

    let applied: AuthorityEventV1 = serde_json::from_slice(bytes("applied_event")?)
        .map_err(|_| "foreign_reference_applied_event_invalid".to_string())?;
    applied
        .validate()
        .map_err(|_| "foreign_reference_applied_event_invalid".to_string())?;
    if applied.id != reference.source.applied_event.id
        || applied.root()? != reference.source.applied_event.root
        || applied.semantic_event_id()? != reference.source.applied_event.semantic_id
        || applied.content.kind != EventKind::FindingSuperseded
        || applied.content.after_hash != reference.source.claim.root
        || payload_text(&applied.content.payload, "claim_id")
            != Some(reference.source.claim.id.as_str())
        || payload_text(&applied.content.payload, "claim_root")
            != Some(reference.source.claim.root.as_str())
        || payload_text(&applied.content.payload, "proposal_id")
            != Some(reference.source.proposal.id.as_str())
        || payload_text(&applied.content.payload, "repository_after")
            != Some(
                reference
                    .source
                    .transition_repository
                    .repository_root
                    .as_str(),
            )
    {
        return Err("foreign_reference_applied_event_mismatch".to_string());
    }

    let decision: AuthorityEventV1 = serde_json::from_slice(bytes("decision_event")?)
        .map_err(|_| "foreign_reference_decision_event_invalid".to_string())?;
    decision
        .validate()
        .map_err(|_| "foreign_reference_decision_event_invalid".to_string())?;
    if decision.id != reference.source.decision_event.id
        || decision.root()? != reference.source.decision_event.root
        || decision.content.kind != EventKind::ReviewAccepted
        || payload_text(&decision.content.payload, "proposal_id")
            != Some(reference.source.proposal.id.as_str())
        || payload_text(&decision.content.payload, "verdict") != Some("accepted")
        || payload_text(&decision.content.payload, "applied_event_id")
            != Some(reference.source.applied_event.semantic_id.as_str())
        || payload_text(&decision.content.payload, "repository_after")
            != Some(
                reference
                    .source
                    .transition_repository
                    .repository_root
                    .as_str(),
            )
    {
        return Err("foreign_reference_decision_event_mismatch".to_string());
    }

    let keyset: AuthorityKeysetV1 = serde_json::from_slice(bytes("authority_keyset")?)
        .map_err(|_| "foreign_reference_authority_keyset_invalid".to_string())?;
    if keyset.root()? != reference.source.authority_keyset_root
        || keyset.frontier_id != reference.source.frontier_id
    {
        return Err("foreign_reference_authority_keyset_mismatch".to_string());
    }
    let envelope: AuthorityEnvelopeV1 = serde_json::from_slice(bytes("authority_record")?)
        .map_err(|_| "foreign_reference_authority_record_invalid".to_string())?;
    let payload = BASE64_STANDARD
        .decode(&envelope.payload)
        .map_err(|_| "foreign_reference_authority_record_invalid".to_string())?;
    let record: AuthorityRecordV1 = serde_json::from_slice(&payload)
        .map_err(|_| "foreign_reference_authority_record_invalid".to_string())?;
    let verified = verify_authority_envelope(
        &envelope,
        &keyset,
        &reference.source.frontier_id,
        record.content.sequence,
        record.content.previous_authority_record_root.as_deref(),
    )
    .map_err(|_| "foreign_reference_authority_signature_invalid".to_string())?;
    if verified.record.record_id != reference.source.authority_record.id
        || verified.record_root != reference.source.authority_record.root
        || verified.record.content.event_ids
            != vec![
                reference.source.applied_event.id.clone(),
                reference.source.decision_event.id.clone(),
            ]
        || verified.record.content.after_event_log_root != predecessor.archived_event_log_root
        || !verified
            .record
            .content
            .semantic_approvals
            .iter()
            .any(|approval| approval.action == "review_accept")
        || !record_delta_matches(
            &verified.record,
            ".vela/repository.json",
            &reference.source.transition_repository.repository_root,
        )
        || !record_delta_matches(
            &verified.record,
            &format!(
                ".vela/authority/events/{}.json",
                reference.source.applied_event.id
            ),
            &reference.source.applied_event.root,
        )
        || !record_delta_matches(
            &verified.record,
            &format!(
                ".vela/authority/events/{}.json",
                reference.source.decision_event.id
            ),
            &reference.source.decision_event.root,
        )
    {
        return Err("foreign_reference_authority_record_mismatch".to_string());
    }
    Ok(())
}

fn require_canonical_json(bytes: &[u8], error: &str) -> Result<(), String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| error.to_string())?;
    if vela_protocol::canonical::to_canonical_bytes(&value)? != bytes {
        return Err(error.to_string());
    }
    Ok(())
}

fn require_transition_claim(
    transition: &Value,
    reference: &ForeignReferenceV1,
) -> Result<(), String> {
    let accepted = transition
        .get("accepted_claims")
        .and_then(Value::as_array)
        .ok_or_else(|| "foreign_reference_transition_repository_invalid".to_string())?;
    if accepted.iter().any(|entry| {
        entry.get("claim_id").and_then(Value::as_str) == Some(reference.source.claim.id.as_str())
            && entry.get("claim_root").and_then(Value::as_str)
                == Some(reference.source.claim.root.as_str())
            && entry.get("standing").and_then(Value::as_str) == Some("accepted")
    }) {
        Ok(())
    } else {
        Err("foreign_reference_transition_claim_missing".to_string())
    }
}

fn require_transition_object(
    transition: &Value,
    field: &str,
    expected: &ExactObjectRef,
) -> Result<(), String> {
    let values = transition
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| "foreign_reference_transition_repository_invalid".to_string())?;
    if values.iter().any(|entry| {
        entry.get("id").and_then(Value::as_str) == Some(expected.id.as_str())
            && entry.get("root").and_then(Value::as_str) == Some(expected.root.as_str())
    }) {
        Ok(())
    } else {
        Err(format!("foreign_reference_transition_{field}_missing"))
    }
}

fn payload_text<'a>(payload: &'a Value, field: &str) -> Option<&'a str> {
    payload.get(field).and_then(Value::as_str)
}

fn record_delta_matches(record: &AuthorityRecordV1, path: &str, after_root: &str) -> bool {
    record
        .content
        .object_delta
        .iter()
        .any(|delta| delta.path == path && delta.after_root.as_deref() == Some(after_root))
}

fn require_role_binding(
    reference: &ForeignReferenceV1,
    role: &str,
    exact_path: &str,
    id: Option<&str>,
    root: &str,
) -> Result<(), String> {
    let Some(object) = reference.objects.iter().find(|object| object.role == role) else {
        if reference.completeness.status == "incomplete"
            && reference
                .completeness
                .missing_roles
                .iter()
                .any(|missing| missing == role)
        {
            return Ok(());
        }
        return Err(format!("foreign_reference_required_role_missing:{role}"));
    };
    if id.is_some_and(|expected| object.id != expected)
        || object.root != root
        || (!exact_path.is_empty() && object.path != exact_path)
    {
        return Err(format!("foreign_reference_role_binding_mismatch:{role}"));
    }
    Ok(())
}

fn require_exact_ref(
    reference: &ExactObjectRef,
    prefix: &str,
    length: usize,
) -> Result<(), String> {
    require_prefixed_hex(&reference.id, prefix, length, "object_id")?;
    require_sha256(&reference.root)
}

fn require_relative_path(value: &str) -> Result<(), String> {
    require_text(value)?;
    let path = Path::new(value);
    if path.is_absolute()
        || value.contains('\\')
        || value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::CurDir | Component::RootDir
            )
        })
    {
        return Err("foreign_reference_object_path_invalid".to_string());
    }
    Ok(())
}

fn require_sha256(value: &str) -> Result<(), String> {
    require_prefixed_hex(value, "sha256:", 64, "sha256")
}

fn require_prefixed_hex(
    value: &str,
    prefix: &str,
    length: usize,
    field: &str,
) -> Result<(), String> {
    let Some(suffix) = value.strip_prefix(prefix) else {
        return Err(format!("foreign_reference_{field}_invalid"));
    };
    require_hex(suffix, length, field)
}

fn require_hex(value: &str, length: usize, field: &str) -> Result<(), String> {
    if value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(format!("foreign_reference_{field}_invalid"))
    }
}

fn require_text(value: &str) -> Result<(), String> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        Err("foreign_reference_text_invalid".to_string())
    } else {
        Ok(())
    }
}

fn canonical_root(value: &impl Serialize) -> Result<String, String> {
    let bytes = vela_protocol::canonical::to_canonical_bytes(value)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}
