//! The two Claim relation vocabularies, bound to the language-neutral fixture.
//!
//! Four competing relation sets were in circulation with no test behind any of
//! them: the seven names PROTOCOL.md declared, the six the repositories retain,
//! the three the correction-impact experiment recognises, and the two the
//! acceptance path actually reads. This test makes the reconciled pair the one
//! a build can fail on.

use std::collections::BTreeMap;

use serde_json::Value;
use vela_protocol::claim_record::{
    CORRECTION_RELATION_KINDS, ClaimAssertion, ClaimRecordV1, ClaimRelation, ClaimRelationClass,
    DESCRIPTIVE_RELATION_KINDS, claim_relation_class,
};

const FIXTURE: &str = include_str!("../../../conformance/fixtures/claim-relation-vocabulary.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("fixture parses")
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("array")
        .iter()
        .map(|entry| entry.as_str().expect("string").to_string())
        .collect()
}

fn class_name(class: ClaimRelationClass) -> &'static str {
    match class {
        ClaimRelationClass::Correction => "correction",
        ClaimRelationClass::Descriptive => "descriptive",
        ClaimRelationClass::Unrecognized => "unrecognized",
    }
}

fn claim_with_relation_kind(kind: &str) -> Result<ClaimRecordV1, String> {
    ClaimRecordV1::build(
        2,
        ClaimAssertion {
            text: "A bounded computation returned no witness.".into(),
            kind: "computational".into(),
        },
        vec![],
        vec![],
        vec![],
        vec![ClaimRelation {
            kind: kind.into(),
            target_claim_id: format!("vcl_{}", "a".repeat(64)),
        }],
        "2026-08-06T00:00:00Z".into(),
        BTreeMap::new(),
    )
}

#[test]
fn the_declared_sets_are_exactly_what_the_crate_exports() {
    let fixture = fixture();
    assert_eq!(
        fixture["schema"],
        "vela.claim-relation-vocabulary-fixture.v1"
    );

    assert_eq!(
        strings(&fixture["correction_relations"]["kinds"]),
        CORRECTION_RELATION_KINDS
    );
    assert_eq!(
        strings(&fixture["descriptive_relations"]["kinds"]),
        DESCRIPTIVE_RELATION_KINDS
    );

    // Only the correction set is authoritative, and only it moves Standing.
    assert_eq!(fixture["correction_relations"]["authoritative"], true);
    assert_eq!(fixture["correction_relations"]["moves_standing"], true);
    assert_eq!(fixture["descriptive_relations"]["authoritative"], false);
    assert_eq!(fixture["descriptive_relations"]["moves_standing"], false);

    // The two sets are disjoint, and each is sorted so drift is a visible diff.
    for kind in DESCRIPTIVE_RELATION_KINDS {
        assert!(
            !CORRECTION_RELATION_KINDS.contains(kind),
            "`{kind}` cannot be both authoritative and descriptive"
        );
    }
    let mut sorted = DESCRIPTIVE_RELATION_KINDS.to_vec();
    sorted.sort_unstable();
    assert_eq!(sorted, DESCRIPTIVE_RELATION_KINDS);
}

#[test]
fn every_classification_case_agrees_with_the_parser() {
    for case in fixture()["classification_cases"]
        .as_array()
        .expect("classification cases are an array")
    {
        let kind = case["kind"].as_str().expect("kind");
        assert_eq!(
            class_name(claim_relation_class(kind)),
            case["class"].as_str().expect("class"),
            "class of `{kind}`"
        );

        // A well-formed kind is accepted whatever its class: an unrecognized
        // relation is retained description, never a parse failure.
        let record = claim_with_relation_kind(kind).expect("well-formed kind is accepted");
        let relation = &record.relations[0];
        assert_eq!(class_name(relation.class()), case["class"]);
        assert_eq!(
            relation.moves_standing(),
            case["class"] == "correction",
            "only the correction algebra moves Standing"
        );
        // Round-trips unchanged: canonicalization is read-side, so recognising
        // a near-miss can never rewrite a retained record's bytes.
        let parsed = ClaimRecordV1::parse(&record.canonical_bytes().expect("canonical bytes"))
            .expect("record round-trips");
        assert_eq!(parsed.relations[0].kind, kind);
        assert_eq!(parsed.claim_id, record.claim_id);
    }
}

#[test]
fn malformed_relation_kinds_fail_closed() {
    for kind in strings(&fixture()["rejected_kinds"]) {
        let error =
            claim_with_relation_kind(&kind).expect_err(&format!("`{kind}` must be rejected"));
        assert!(
            error.contains("relations.kind"),
            "`{kind}` was rejected for the wrong reason: {error}"
        );
    }
}

#[test]
fn every_retained_relation_kind_in_the_census_is_declared() {
    let fixture = fixture();
    let census = fixture["census"]["relations_by_kind"]
        .as_object()
        .expect("census is an object");
    assert!(!census.is_empty());

    for (kind, count) in census {
        assert!(
            count.as_u64().expect("count is a number") > 0,
            "`{kind}` is listed with no uses"
        );
        assert_ne!(
            claim_relation_class(kind),
            ClaimRelationClass::Unrecognized,
            "`{kind}` is retained in a repository and declared nowhere"
        );
        claim_with_relation_kind(kind).unwrap_or_else(|error| {
            panic!("retained kind `{kind}` no longer parses: {error}");
        });
    }

    // The names that were declared and never written stay listed, so that
    // reviving one is a deliberate edit rather than a silent drift.
    for kind in strings(&fixture["census"]["never_written"]["kinds"]) {
        assert!(
            !census.contains_key(&kind),
            "`{kind}` is recorded as never written but appears in the census"
        );
    }
}
