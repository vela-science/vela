//! `vela.repository-projection.v1`: one verified current-checkout read model.
//!
//! This is a derived read document, not a signed Protocol object. It gives a
//! consumer the scientific state Core already derives for `status`, `why`,
//! `review show`, and the Decision Inbox without requiring that consumer to
//! decode `.vela/repository.json`, DSSE payloads, or authority Events itself.
//! It is deliberately about one checkout. Git history selection, product
//! presentation, graph/search layout, and source-specific joins stay with the
//! consumer that owns them.
//!
//! Like `vela.status.v4`, this document is open to additive fields. Every v1
//! field is nevertheless present on the wire; nullable values are explicit
//! nulls. Embedded `record`, `envelope`, and `payload` values are exact closed
//! canonical objects whose own schemas remain authoritative.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::status::{
    StatusActions, StatusCounts, StatusDecisionInbox, StatusGit, StatusIntegrity, StatusRoots,
};

pub const REPOSITORY_PROJECTION_V1_SCHEMA: &str = "vela.repository-projection.v1";
pub const REPOSITORY_PROJECTION_COMMAND: &str = "projection";
pub const REPOSITORY_PROJECTION_AUTHORITY_EFFECT: &str = "none";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionRepositoryV1 {
    #[schemars(schema_with = "crate::wire_schema::repository_id")]
    pub repository_id: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub name: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub profile_root: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub origin_id: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub origin_root: String,
    pub origin_generation: u64,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub initial_object_set_root: String,
    #[schemars(schema_with = "crate::wire_schema::safe_relative_path")]
    pub repository_index_path: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub repository_root: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub authority_keyset_root: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub authority_policy_root: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub authority_record_root: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub authority_event_log_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionEventRefV1 {
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub authority_event_id: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub authority_event_root: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub semantic_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionTransitionV1 {
    #[schemars(schema_with = "crate::wire_schema::projection_transition_event_kind")]
    pub authority_event_kind: String,
    #[schemars(
        required,
        schema_with = "crate::wire_schema::nullable_correction_relation_kind"
    )]
    pub relation_kind: Option<String>,
    #[schemars(required)]
    pub predecessor_claim_id: Option<String>,
    #[schemars(required, schema_with = "crate::wire_schema::nullable_sha256_root")]
    pub predecessor_claim_root: Option<String>,
    #[schemars(required)]
    pub successor_claim_id: Option<String>,
    #[schemars(required, schema_with = "crate::wire_schema::nullable_sha256_root")]
    pub successor_claim_root: Option<String>,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub proposal_id: String,
    pub decision_event: ProjectionEventRefV1,
    #[schemars(required)]
    pub applied_event: Option<ProjectionEventRefV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionClaimV1 {
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub claim_id: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub claim_root: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub source_path: String,
    pub active: bool,
    #[schemars(schema_with = "crate::wire_schema::projection_claim_standing")]
    pub standing: String,
    #[schemars(
        required,
        schema_with = "crate::wire_schema::nullable_projection_proposal_status"
    )]
    pub proposal_status: Option<String>,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub assertion: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub assertion_kind: String,
    pub record: Value,
    #[schemars(required)]
    pub transition: Option<ProjectionTransitionV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionDecisionV1 {
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub verdict: String,
    #[schemars(schema_with = "crate::wire_schema::timestamp")]
    pub decided_at: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub reason: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub actor_id: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub actor_class: String,
    #[schemars(required, schema_with = "crate::wire_schema::nullable_text")]
    pub session_ref: Option<String>,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub authority_principal_id: String,
    #[schemars(required, schema_with = "crate::wire_schema::nullable_sha256_root")]
    pub repository_before: Option<String>,
    #[schemars(required, schema_with = "crate::wire_schema::nullable_sha256_root")]
    pub repository_after: Option<String>,
    pub decision_event: ProjectionEventRefV1,
    #[schemars(required)]
    pub applied_event: Option<ProjectionEventRefV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionProposalV1 {
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub proposal_id: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub proposal_root: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub source_path: String,
    #[schemars(schema_with = "crate::wire_schema::projection_proposal_status")]
    pub status: String,
    #[schemars(schema_with = "crate::wire_schema::projection_claim_standing")]
    pub subject_standing: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub submission_id: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub submission_root: String,
    pub verification_record_ids: Vec<String>,
    pub record: Value,
    #[schemars(required)]
    pub decision: Option<ProjectionDecisionV1>,
    #[schemars(required)]
    pub withdrawal: Option<Value>,
    #[schemars(required)]
    pub decision_inbox_entry: Option<Value>,
    pub consequence: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionAuthenticationV1 {
    pub signature_verified: bool,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub actor_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionAuthenticatedObjectV1 {
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub object_id: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub object_root: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub source_path: String,
    pub envelope: Value,
    pub payload: Value,
    pub authentication: ProjectionAuthenticationV1,
    #[schemars(required)]
    pub review_method: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionArtifactV1 {
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub artifact_id: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub artifact_root: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub source_path: String,
    pub byte_length: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionAuthorityEventV1 {
    pub event: ProjectionEventRefV1,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub kind: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub actor_id: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub target_id: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub target_type: String,
    #[schemars(schema_with = "crate::wire_schema::timestamp")]
    pub timestamp: String,
    pub record: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionHandoffV1 {
    pub accepted_claim_ids: Vec<String>,
    pub unassessed_claim_ids: Vec<String>,
    pub retired_claim_ids: Vec<String>,
    pub pending_proposal_ids: Vec<String>,
    pub correction_successor_ids: Vec<String>,
    pub exact_next_actions: Vec<String>,
    pub failed_routes: Vec<String>,
    pub limitations: Vec<String>,
    pub nonclaims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryProjectionV1 {
    #[schemars(schema_with = "crate::wire_schema::repository_projection_schema_tag")]
    pub schema: String,
    #[schemars(schema_with = "crate::wire_schema::ok_true")]
    pub ok: bool,
    #[schemars(schema_with = "crate::wire_schema::repository_projection_command_tag")]
    pub command: String,
    #[schemars(schema_with = "crate::wire_schema::authority_effect_none_tag")]
    pub authority_effect: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub reader_version: String,
    pub repository: ProjectionRepositoryV1,
    pub git: StatusGit,
    pub integrity: StatusIntegrity,
    pub roots: StatusRoots,
    pub counts: StatusCounts,
    pub decision_inbox_summary: StatusDecisionInbox,
    pub actions: StatusActions,
    pub claims: Vec<ProjectionClaimV1>,
    pub proposals: Vec<ProjectionProposalV1>,
    pub submissions: Vec<ProjectionAuthenticatedObjectV1>,
    pub verifications: Vec<ProjectionAuthenticatedObjectV1>,
    pub artifacts: Vec<ProjectionArtifactV1>,
    pub authority_events: Vec<ProjectionAuthorityEventV1>,
    pub correction_impacts: Vec<Value>,
    pub decision_inbox: Value,
    pub handoff: ProjectionHandoffV1,
}
