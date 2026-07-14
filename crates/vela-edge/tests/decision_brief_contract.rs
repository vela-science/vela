use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_edge::decision_brief::{
    DecisionBrief, DecisionBriefInput, ReceiptMaterial, ReviewRoute, build_decision_brief,
};
use vela_protocol::events::{self, StateTarget};
use vela_protocol::project::{self, Project};
use vela_protocol::proposals;
use vela_protocol::test_support::{make_finding, make_project};

const OBSERVED_AT: &str = "2026-07-13T13:00:00Z";
const CREATED_AT: &str = "2026-07-13T12:35:00Z";
const FIXTURE_ACTOR: &str = "agent:decision-brief-fixture";

fn fixed_project(name: &str, findings: Vec<vela_protocol::bundle::FindingBundle>) -> Project {
    let mut project = make_project(name, findings);
    project.project.compiled_at = "2026-07-13T12:00:00Z".to_string();
    project.project.compiler = "vela/decision-brief-fixture.v1".to_string();

    let genesis = project
        .events
        .first_mut()
        .expect("test-support projects have a genesis event");
    genesis.timestamp = "2026-07-13T12:00:00Z".to_string();
    genesis.actor.id = "vela/decision-brief-fixture.v1".to_string();
    genesis.payload["compiled_at"] = json!("2026-07-13T12:00:00Z");
    genesis.payload["creator"] = json!("vela/decision-brief-fixture.v1");
    genesis.payload["schema_version"] = json!("decision-brief-fixture.v1");
    genesis.id = events::compute_event_id(genesis);
    project.frontier_id = project::frontier_id_from_genesis(&project.events);
    project
}

fn finding_value(id: &str, assertion_type: &str, claim: &str) -> Value {
    let mut value = serde_json::to_value(make_finding(id, 0.3, assertion_type)).unwrap();
    value["assertion"]["text"] = json!(claim);
    value
}

fn install_proposal(
    project: &mut Project,
    kind: &str,
    target_id: &str,
    payload: Value,
    source_refs: Vec<String>,
    caveats: Vec<String>,
    reason: &str,
) -> String {
    let proposal = proposals::new_proposal_at(
        kind,
        StateTarget {
            r#type: "finding".to_string(),
            id: target_id.to_string(),
        },
        FIXTURE_ACTOR,
        "agent",
        reason,
        payload,
        source_refs,
        caveats,
        CREATED_AT,
    );
    let id = proposal.id.clone();
    project.proposals.push(proposal);
    id
}

fn ordinary_brief() -> DecisionBrief {
    let target = "vf_decision_brief_ordinary";
    let claim = "A bounded computational note is ready for human review.";
    let mut project = fixed_project("Decision brief ordinary fixture", vec![]);
    let proposal_id = install_proposal(
        &mut project,
        "finding.note",
        target,
        json!({"finding": finding_value(target, "computational", claim)}),
        vec!["urn:source:ordinary".to_string()],
        vec!["The note is scoped to the declared bounded case.".to_string()],
        "record a bounded note",
    );

    build_decision_brief(
        &project,
        DecisionBriefInput {
            proposal_id: &proposal_id,
            receipt: ReceiptMaterial::missing("receipt_not_applicable"),
            route: ReviewRoute::human_only(
                "proposal_kind_requires_human_review",
                "this proposal kind is intentionally reviewed by a human",
            ),
            observed_at: OBSERVED_AT,
            replay_ok: true,
            publication: None,
        },
    )
    .unwrap()
}

fn critical_warning_brief() -> DecisionBrief {
    let target = "vf_decision_brief_contested";
    let claim = "The contested claim should remain visible during review.";
    let mut existing = make_finding(target, 0.3, "computational");
    existing.flags.contested = true;
    let mut project = fixed_project("Decision brief critical warning fixture", vec![existing]);
    let proposal_id = install_proposal(
        &mut project,
        "finding.note",
        target,
        json!({"finding": finding_value(target, "computational", claim)}),
        vec!["urn:source:contested-review".to_string()],
        vec!["An active challenge must be resolved by the reviewer.".to_string()],
        "record review context without resolving the challenge",
    );

    build_decision_brief(
        &project,
        DecisionBriefInput {
            proposal_id: &proposal_id,
            receipt: ReceiptMaterial::missing("receipt_not_applicable"),
            route: ReviewRoute::human_only(
                "active_challenge_requires_human_review",
                "the active challenge requires an explicit human decision",
            ),
            observed_at: OBSERVED_AT,
            replay_ok: true,
            publication: None,
        },
    )
    .unwrap()
}

fn missing_brief() -> DecisionBrief {
    let target = "vf_decision_brief_missing";
    let claim = "A receipt-bound computational finding is proposed.";
    let declared_root = format!("sha256:{}", "4".repeat(64));
    let mut project = fixed_project("Decision brief missing fixture", vec![]);
    let proposal_id = install_proposal(
        &mut project,
        "finding.add",
        target,
        json!({
            "finding": finding_value(target, "computational", claim),
            "vela_submission": {
                "schema": "vela.submission-links.internal.v1",
                "receipt_root": declared_root,
                "receipt_path": format!("records/receipts/sha256/{}.json", "4".repeat(64)),
                "record_id": "vrc_decision_brief_missing",
                "operation_id": format!("vop_{}", "5".repeat(64))
            }
        }),
        vec!["urn:source:missing-receipt".to_string()],
        vec!["The receipt must be recovered before acceptance.".to_string()],
        "land a receipt-bound finding for review",
    );

    build_decision_brief(
        &project,
        DecisionBriefInput {
            proposal_id: &proposal_id,
            receipt: ReceiptMaterial::missing("receipt_not_found"),
            route: ReviewRoute::unavailable(
                "broken",
                "the coherent policy route could not be reconstructed",
            ),
            observed_at: OBSERVED_AT,
            replay_ok: true,
            publication: None,
        },
    )
    .unwrap()
}

fn restricted_evidence_brief() -> DecisionBrief {
    let target = "vf_decision_brief_restricted";
    let claim = "A theoretical claim depends on evidence unavailable in this review context.";
    let declared_root = format!("sha256:{}", "6".repeat(64));
    // The locator is a safe descriptor, not restricted content. Its length
    // exercises the bounded rendering while raw_references_root binds the
    // complete source value.
    let restricted_locator = format!(
        "restricted://review-vault/safe-descriptor/{}",
        "bounded-segment-".repeat(40)
    );
    let mut project = fixed_project("Decision brief restricted evidence fixture", vec![]);
    let proposal_id = install_proposal(
        &mut project,
        "finding.add",
        target,
        json!({
            "finding": finding_value(target, "theoretical", claim),
            "vela_submission": {
                "schema": "vela.submission-links.internal.v1",
                "receipt_root": declared_root,
                "receipt_path": format!("records/receipts/sha256/{}.json", "6".repeat(64)),
                "record_id": "vrc_decision_brief_restricted",
                "operation_id": format!("vop_{}", "7".repeat(64))
            }
        }),
        vec![restricted_locator],
        vec!["Evidence access is restricted; no evidentiary authority is inferred.".to_string()],
        "route restricted evidence to an authorized human reviewer",
    );

    build_decision_brief(
        &project,
        DecisionBriefInput {
            proposal_id: &proposal_id,
            receipt: ReceiptMaterial::missing("restricted_evidence_not_available_to_reviewer"),
            route: ReviewRoute::human_only(
                "restricted_evidence_requires_authorized_human_review",
                "restricted evidence requires an authorized human review",
            ),
            observed_at: OBSERVED_AT,
            replay_ok: true,
            publication: None,
        },
    )
    .unwrap()
}

fn generated_cases() -> [(&'static str, DecisionBrief); 4] {
    [
        ("ordinary", ordinary_brief()),
        ("critical-warning", critical_warning_brief()),
        ("missing", missing_brief()),
        ("restricted-evidence", restricted_evidence_brief()),
    ]
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/decision-brief-testing-v1")
}

fn canonical_sha256(value: &Value) -> String {
    let bytes = vela_protocol::canonical::to_canonical_bytes(value).unwrap();
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn frozen_roots(name: &str) -> (&'static str, &'static str) {
    match name {
        "ordinary" => (
            "sha256:fa57bf7019b5f012b548842f0875938ebf8d03104491d23ecf252ab6fe50a27c",
            "sha256:9bc7756fab626c05d6fbddfd5c4efee63586c71ab5e1bb6c95dd2c34c25d66c5",
        ),
        "critical-warning" => (
            "sha256:f82bf925239821a0056cde93a8887cf3f205f45c0323e2fa67db23930a1c6329",
            "sha256:7671f9929c92454461aceec3c812a6f8ea41f5755de2c93615440fdf64d9df53",
        ),
        "missing" => (
            "sha256:0c5996b6a29de835b6050faf031bb7008fb2deaa950a97a065ba4e9941b602ac",
            "sha256:1548dbb556ffc3a16b58476fa78452bfc7fc1a6393632c5a5f15d6bbce4e200b",
        ),
        "restricted-evidence" => (
            "sha256:9b3fe88c9ba14e9507c48b64580ceccc0153ceb5101ac70441afece012d3e047",
            "sha256:c7740f86773cef7e121ea3a58d0327a90b17ed515b762c3a938ed3b6a3baea6c",
        ),
        other => panic!("no frozen roots for {other}"),
    }
}

fn assert_sha256_root(value: &Value, field: &str) {
    let root = value
        .as_str()
        .unwrap_or_else(|| panic!("{field} must be a string root"));
    assert_eq!(root.len(), 71, "{field} has the wrong root length");
    let digest = root
        .strip_prefix("sha256:")
        .unwrap_or_else(|| panic!("{field} must be sha256-prefixed"));
    assert!(
        digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{field} must contain lowercase hexadecimal"
    );
}

fn assert_no_generic_authority_keys(value: &Value, path: &str) {
    match value {
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                assert_no_generic_authority_keys(value, &format!("{path}[{index}]"));
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                assert!(
                    key != "signed" && key != "trusted",
                    "generic authority key {path}.{key} is forbidden"
                );
                assert_no_generic_authority_keys(value, &format!("{path}.{key}"));
            }
        }
        _ => {}
    }
}

fn assert_bounded_facet_value(value: &Value, depth: usize) {
    assert!(depth <= 9, "facet projection exceeded its depth bound");
    match value {
        Value::String(text) => {
            assert!(text.len() <= 1027, "facet string exceeded its byte bound");
        }
        Value::Array(values) => {
            assert!(values.len() <= 32, "facet array exceeded its item bound");
            for value in values {
                assert_bounded_facet_value(value, depth + 1);
            }
        }
        Value::Object(values) => {
            assert!(values.len() <= 64, "facet object exceeded its field bound");
            for value in values.values() {
                assert_bounded_facet_value(value, depth + 1);
            }
        }
        _ => {}
    }
}

fn assert_contract_shape(value: &Value) {
    let object = value.as_object().expect("brief must be an object");
    for section in ["change", "basis", "impact", "authority", "audit"] {
        assert!(
            object.get(section).is_some_and(Value::is_object),
            "required section {section} is absent"
        );
    }
    assert_eq!(value["schema"], json!("vela.decision-brief.testing.v1"));
    assert_eq!(value["stability"], json!("testing"));

    let facets = value["facets"].as_object().expect("facets must be a map");
    assert!(!facets.is_empty(), "fixtures must exercise a typed facet");
    let keys = facets.keys().collect::<Vec<_>>();
    assert!(keys.is_sorted(), "facet map must serialize in sorted order");
    for (name, facet) in facets {
        assert!(
            facet["schema"]
                .as_str()
                .is_some_and(|schema| schema.starts_with("vela.decision-brief.facet.")),
            "facet {name} must carry a typed schema"
        );
        assert!(facet["critical"].is_boolean());
        assert!(facet["truncated"].is_boolean());
        assert_sha256_root(&facet["full_root"], &format!("facets.{name}.full_root"));
        assert_bounded_facet_value(&facet["data"], 1);
    }

    let actions = value["authority"]["actions"]
        .as_array()
        .expect("actions must be an array");
    assert_eq!(actions.len(), 2);
    for action_name in ["accept", "reject"] {
        let action = actions
            .iter()
            .find(|candidate| candidate["action"] == json!(action_name))
            .unwrap_or_else(|| panic!("missing {action_name} action"));
        let eligibility = action["eligibility"]
            .as_str()
            .expect("action eligibility must be typed");
        assert!(eligibility == "available" || eligibility == "blocked");
        let reasons = action["reasons"].as_array().expect("reasons must be typed");
        assert_eq!(eligibility == "available", reasons.is_empty());
        if action_name == "reject" {
            assert_eq!(eligibility, "available");
        }
    }

    let audit = &value["audit"];
    for field in [
        "proposal_root",
        "decision_facts_root",
        "policy_input_root",
        "policy_result_root",
        "raw_references_root",
        "missing_root",
    ] {
        assert_sha256_root(&audit[field], &format!("audit.{field}"));
    }
    let references = audit["raw_references"]
        .as_array()
        .expect("raw references must be typed");
    assert!(references.len() <= 64);
    for reference in references {
        assert!(reference.as_str().unwrap().len() <= 515);
    }
    assert!(value["missing"].as_array().unwrap().len() <= 16);
    for truncation in audit["truncations"].as_array().unwrap() {
        assert_sha256_root(&truncation["full_root"], "audit.truncations.full_root");
        assert!(truncation["omitted_bytes"].as_u64().unwrap() > 0);
    }
    assert_no_generic_authority_keys(value, "$brief");
}

#[test]
fn golden_fixtures_match_public_builder() {
    for (name, brief) in generated_cases() {
        let path = fixture_dir().join(format!("{name}.json"));
        let frozen: Value = serde_json::from_slice(
            &std::fs::read(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
        )
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
        let generated = serde_json::to_value(brief).unwrap();
        assert_eq!(generated, frozen, "fixture drift: {}", path.display());
        assert_eq!(
            vela_protocol::canonical::to_canonical_bytes(&generated).unwrap(),
            vela_protocol::canonical::to_canonical_bytes(&frozen).unwrap(),
            "canonical byte drift: {}",
            path.display()
        );
        let (canonical_root, decision_facts_root) = frozen_roots(name);
        assert_eq!(canonical_sha256(&frozen), canonical_root);
        assert_eq!(
            frozen["audit"]["decision_facts_root"],
            json!(decision_facts_root)
        );
        assert_contract_shape(&frozen);
    }
}

#[test]
fn golden_cases_cover_critical_missing_and_restricted_evidence() {
    let load = |name: &str| -> Value {
        let path = fixture_dir().join(format!("{name}.json"));
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    };

    let ordinary = load("ordinary");
    assert!(ordinary["missing"].as_array().unwrap().is_empty());
    assert!(
        ordinary["impact"]["critical_warnings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        ordinary["authority"]["actions"][0]["eligibility"],
        json!("available")
    );

    let critical = load("critical-warning");
    assert_eq!(critical["facets"]["challenge"]["critical"], json!(true));
    assert!(
        critical["impact"]["critical_warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning["code"] == json!("active_challenge"))
    );

    let missing = load("missing");
    assert!(!missing["missing"].as_array().unwrap().is_empty());
    assert_eq!(missing["authority"]["route"], json!("broken"));
    assert_eq!(
        missing["authority"]["actions"][0]["eligibility"],
        json!("blocked")
    );

    let restricted = load("restricted-evidence");
    assert_eq!(
        restricted["facets"]["formal_fidelity"]["critical"],
        json!(true)
    );
    assert!(
        restricted["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| {
                fact["field"] == json!("basis.receipt")
                    && fact["reason"] == json!("restricted_evidence_not_available_to_reviewer")
            })
    );
    assert!(
        restricted["audit"]["raw_references"].as_array().unwrap()[1]
            .as_str()
            .unwrap()
            .ends_with('…')
    );
    assert_eq!(
        restricted["audit"]["truncations"][0]["field"],
        json!("audit.raw_references[1]")
    );
    assert_eq!(restricted["authority"]["route"], json!("defer"));
}

#[test]
fn published_schema_freezes_sections_facets_and_actions() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/vela.decision-brief.testing.v1.schema.json");
    let schema: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        schema["$schema"],
        json!("https://json-schema.org/draft/2020-12/schema")
    );
    assert_eq!(
        schema["properties"]["schema"]["const"],
        json!("vela.decision-brief.testing.v1")
    );
    let required = schema["required"].as_array().unwrap();
    for field in [
        "change",
        "basis",
        "impact",
        "authority",
        "audit",
        "missing",
        "facets",
    ] {
        assert!(
            required.contains(&json!(field)),
            "schema must require {field}"
        );
    }
    assert_eq!(
        schema["properties"]["facets"]["additionalProperties"]["$ref"],
        json!("#/$defs/typed_facet")
    );
    assert_eq!(
        schema["$defs"]["action"]["properties"]["eligibility"]["enum"],
        json!(["available", "blocked"])
    );
}
