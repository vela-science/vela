//! Every authorization Cedar ever published, recomputed by the closed evaluator.
//!
//! `docs/PORTABLE_WAIST_CAMPAIGN.md` Cut C asks for exactly this before Cedar
//! is deleted: "recompute every historical Allow result with the closed
//! profile" and "prove parity and negative boundary cases". The corpus for it
//! has existed since 2026-08-02 in
//! `conformance/fixtures/epoch1/authorization-profile-parity.json` and, as
//! `AGENTS.md` says in as many words, nothing read it — not `crates/`, not
//! `conformance/`, not `scripts/`, not `.github/`. This is the reader.
//!
//! ## What carries over, and what cannot
//!
//! The corpus is epoch-1. It was measured against the four repositories that
//! ADR 0039 archived, so it speaks their vocabulary: `frontier_id` rather than
//! `repository_id`, `"resource_type": "frontier"`, `vfr_` and sixteen hex
//! digits rather than `vrepo_` and thirty-two. Its README is explicit that it
//! is retained rather than migrated, "because that is the shape the
//! repositories actually have".
//!
//! So the frozen roots beside each case cannot be reproduced and this test does
//! not pretend to reproduce them. A root is taken over field names, and the
//! field names were renamed by ADR 0039 and the identifier widened by the
//! 128-bit change; a test that claimed root parity across that would be
//! claiming the rename never happened.
//!
//! What does carry over is the thing Cut C actually asks about: the decision.
//! Whether a principal holding a role may request an action on a resource does
//! not depend on how the repository's identifier is spelled, so each retained
//! case is translated into the current vocabulary by the one deterministic rule
//! stated in `current_repository_id` and re-decided. Seven retained Allows and
//! seven negative boundary mutations, all seven of which Cedar never published
//! because no denied evaluation was ever written.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::Value;
use vela_authority::{
    AuthorityActionV1, AuthorityMemberV1, AuthorityResourceTypeV1, AuthorityRoleV1,
    AuthorizationDecisionV1, AuthorizationModelV1, AuthorizationReasonV1, AuthorizationRequestV1,
    AuthorizationResourceV1, PrincipalClass, evaluate_authorization_v1,
};

const PROFILE: &str = "vela.repository-authorization.v1";
const MODEL_SCHEMA: &str = "vela.authorization-model.v1";
const REQUEST_SCHEMA: &str = "vela.authorization-request.v1";

fn corpus() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/epoch1/authorization-profile-parity.json");
    serde_json::from_slice(&std::fs::read(path).expect("read the epoch-1 parity corpus"))
        .expect("the parity corpus is JSON")
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("parity corpus entry has no `{key}`"))
}

/// Translate a retired `vfr_` identifier into the current `vrepo_` shape.
///
/// One rule, stated once: keep the sixteen measured digits and pad the width
/// the 128-bit change added. It is a spelling, not a claim — no epoch-1
/// repository has a `vrepo_` identifier, and none ever will, because all four
/// are archived. What the translation preserves is exactly what the decision
/// depends on: two repositories that differed still differ, and one that
/// matched still matches.
fn current_repository_id(frontier_id: &str) -> String {
    let body = frontier_id
        .strip_prefix("vfr_")
        .expect("an epoch-1 identifier carries the retired prefix");
    format!("vrepo_{body}{}", "0".repeat(32 - body.len()))
}

fn role(value: &str) -> AuthorityRoleV1 {
    match value {
        "administrator" => AuthorityRoleV1::Administrator,
        "reviewer" => AuthorityRoleV1::Reviewer,
        other => panic!("the closed profile has no role `{other}`"),
    }
}

fn action(value: &str) -> AuthorityActionV1 {
    match value {
        "authority_initialize" => AuthorityActionV1::AuthorityInitialize,
        "authority_rotate" => AuthorityActionV1::AuthorityRotate,
        "authority_close" => AuthorityActionV1::AuthorityClose,
        "authority_model_update" => AuthorityActionV1::AuthorityModelUpdate,
        "review_accept" => AuthorityActionV1::ReviewAccept,
        "review_reject" => AuthorityActionV1::ReviewReject,
        other => panic!("the closed profile has no action `{other}`"),
    }
}

/// The retired reason token beside each negative case, in current spelling.
///
/// Only two moved, and both for the ADR 0039 rename the module comment on
/// `AuthorizationReasonV1` already records.
fn reason(value: &str) -> AuthorizationReasonV1 {
    match value {
        "unknown_member" => AuthorizationReasonV1::UnknownMember,
        "principal_class_mismatch" => AuthorizationReasonV1::PrincipalClassMismatch,
        "role_action_mismatch" => AuthorizationReasonV1::RoleActionMismatch,
        "frontier_mismatch" => AuthorizationReasonV1::RepositoryMismatch,
        "resource_frontier_mismatch" => AuthorizationReasonV1::ResourceRepositoryMismatch,
        "resource_type_mismatch" => AuthorizationReasonV1::ResourceTypeMismatch,
        "recovery_session_forbidden" => AuthorizationReasonV1::RecoverySessionForbidden,
        other => panic!("the closed profile has no reason `{other}`"),
    }
}

/// The model a retained case was decided under, in current vocabulary.
fn model_for(corpus: &Value, source_repository: &str) -> AuthorizationModelV1 {
    let entry = corpus["models"]
        .as_array()
        .expect("the corpus carries its models")
        .iter()
        .find(|entry| text(entry, "source_repository") == source_repository)
        .unwrap_or_else(|| panic!("the corpus has no model for {source_repository}"));
    let model = &entry["model"];
    AuthorizationModelV1 {
        schema: MODEL_SCHEMA.into(),
        profile: PROFILE.into(),
        repository_id: current_repository_id(text(model, "frontier_id")),
        members: model["members"]
            .as_array()
            .expect("a model carries its members")
            .iter()
            .map(|member| AuthorityMemberV1 {
                principal_id: text(member, "principal_id").into(),
                principal_class: match text(member, "principal_class") {
                    "human" => PrincipalClass::Human,
                    other => panic!("the closed profile has no principal class `{other}`"),
                },
                role: role(text(member, "role")),
            })
            .collect(),
        previous_model_root: model["previous_model_root"].as_str().map(str::to_string),
    }
}

fn request_for(case: &Value, model: &AuthorizationModelV1) -> AuthorizationRequestV1 {
    let repository_id = current_repository_id(text(case, "frontier_id"));
    let resource_type = match text(case, "resource_type") {
        // The retired token for the authority boundary ADR 0039 renamed.
        "frontier" => AuthorityResourceTypeV1::Repository,
        "proposal" => AuthorityResourceTypeV1::Proposal,
        other => panic!("the closed profile has no resource type `{other}`"),
    };
    let resource_id = match resource_type {
        AuthorityResourceTypeV1::Repository => current_repository_id(text(case, "resource_id")),
        AuthorityResourceTypeV1::Proposal => text(case, "resource_id").to_string(),
    };
    AuthorizationRequestV1 {
        schema: REQUEST_SCHEMA.into(),
        profile: PROFILE.into(),
        model_root: model.root().expect("the translated model roots"),
        repository_id: repository_id.clone(),
        principal_id: text(case, "principal_id_override")
            .to_string()
            .into_boxed_str()
            .into(),
        principal_class: PrincipalClass::Human,
        action: action(text(case, "action")),
        resource: AuthorizationResourceV1 {
            repository_id,
            resource_type,
            resource_id,
        },
        authentication_root: text(case, "authentication_root").into(),
        transaction_read_set_root: text(case, "transaction_read_set_root").into(),
        intent_digest: text(case, "intent_digest").into(),
        recovery_recent: false,
    }
}

/// Each case names its principal only through the corpus-level field, so this
/// puts it where `request_for` reads it and leaves the retained entry alone.
fn case_with_principal(case: &Value, principal_id: &str) -> Value {
    let mut case = case.clone();
    case["principal_id_override"] = Value::String(principal_id.into());
    case
}

#[test]
fn every_retained_cedar_allow_is_reproduced_by_the_closed_evaluator() {
    let corpus = corpus();
    let principal = text(&corpus, "principal_id");
    let cases = corpus["cases"]
        .as_array()
        .expect("the corpus carries cases");

    let mut decided = BTreeSet::new();
    for case in cases {
        let source = text(case, "source_repository");
        let model = model_for(&corpus, source);
        let request = request_for(&case_with_principal(case, principal), &model);
        let evaluation = evaluate_authorization_v1(&model, &request).unwrap_or_else(|error| {
            panic!(
                "{source} {} did not evaluate: {error}",
                text(case, "record_id")
            )
        });

        assert_eq!(
            evaluation.decision,
            AuthorizationDecisionV1::Allow,
            "{source} {} was published as an Allow and the closed profile denies it",
            text(case, "record_id")
        );
        assert_eq!(
            evaluation.reason,
            AuthorizationReasonV1::MemberRoleAuthorized,
            "{source} {} was allowed for the wrong reason",
            text(case, "record_id")
        );
        assert_eq!(
            evaluation.matched_role,
            Some(action(text(case, "action")).required_role()),
            "{source} {} was allowed under the wrong role",
            text(case, "record_id")
        );
        decided.insert(text(case, "record_id").to_string());
    }

    // Seven retained transactions across four repositories. Pinned so the
    // corpus cannot shrink into a test that passes by covering nothing.
    assert_eq!(decided.len(), 7, "the retained Allow corpus changed size");
    assert_eq!(
        cases
            .iter()
            .map(|case| text(case, "source_repository").to_string())
            .collect::<BTreeSet<_>>()
            .len(),
        4,
        "the retained Allow corpus no longer covers all four repositories"
    );
}

#[test]
fn every_negative_boundary_denies_for_its_exact_reason() {
    let corpus = corpus();
    let principal = text(&corpus, "principal_id");
    let case = corpus["cases"]
        .as_array()
        .and_then(|cases| cases.first())
        .expect("the corpus carries at least one case");
    let model = model_for(&corpus, text(case, "source_repository"));
    let base = request_for(&case_with_principal(case, principal), &model);

    let negatives = corpus["negative_cases"]
        .as_array()
        .expect("the corpus carries its negative boundary cases");
    let other_repository = current_repository_id("vfr_ffffffffffffffff");

    for negative in negatives {
        let mutation = text(negative, "mutation");
        let mut request = base.clone();
        let mut model = model.clone();
        match mutation {
            "unbound_principal" => request.principal_id = "local:device-unbound|uid:0".into(),
            "machine_principal" => request.principal_class = PrincipalClass::Workload,
            /* The role has to move without the resource moving with it.
            Switching the action to `review_accept` would also change the
            required resource type, and the request would deny on that first —
            a boundary case that passes for a reason other than the one it
            names. So the model loses the administrator membership instead,
            leaving an administrator action requested by a reviewer. */
            "wrong_role" => {
                model
                    .members
                    .retain(|member| member.role != AuthorityRoleV1::Administrator);
                request.model_root = model.root().unwrap();
            }
            "frontier_mismatch" | "wrong_frontier" => {
                request.repository_id = other_repository.clone();
                request.resource.repository_id = other_repository.clone();
            }
            "wrong_resource_frontier" => request.resource.repository_id = other_repository.clone(),
            "wrong_resource_type" => {
                request.resource.resource_type = AuthorityResourceTypeV1::Proposal;
                request.resource.resource_id = format!("vpr_{}", "0".repeat(16));
            }
            "recovery_recent" => request.recovery_recent = true,
            other => panic!("the corpus carries an unhandled mutation `{other}`"),
        }

        let evaluation = evaluate_authorization_v1(&model, &request)
            .unwrap_or_else(|error| panic!("{mutation} did not evaluate: {error}"));
        assert_eq!(
            evaluation.decision,
            AuthorizationDecisionV1::Deny,
            "{mutation} was allowed"
        );
        assert_eq!(
            evaluation.reason,
            reason(text(negative, "expected_reason")),
            "{mutation} denied for the wrong reason"
        );
        assert_eq!(evaluation.matched_role, None);
    }

    assert_eq!(
        negatives.len(),
        7,
        "the negative boundary corpus changed size"
    );
}

/// The base request the negatives mutate is itself an Allow.
///
/// Without this, a mutation that changed nothing would still deny for some
/// unrelated reason and the assertions above would read as if the boundary
/// held.
#[test]
fn the_unmutated_negative_baseline_is_allowed() {
    let corpus = corpus();
    let principal = text(&corpus, "principal_id");
    let case = corpus["cases"].as_array().unwrap().first().unwrap();
    let model = model_for(&corpus, text(case, "source_repository"));
    let request = request_for(&case_with_principal(case, principal), &model);

    let evaluation = evaluate_authorization_v1(&model, &request).unwrap();
    assert_eq!(evaluation.decision, AuthorizationDecisionV1::Allow);
}
