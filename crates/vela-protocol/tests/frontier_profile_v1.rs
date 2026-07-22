use vela_protocol::frontier_profile::{FRONTIER_PROFILE_SCHEMA_V1, FrontierProfileV1};

const PROFILE_A: &str = r#"
schema: vela.frontier-profile.v1
frontier_id: vfr_0123456789abcdef
name: Example Frontier
summary: A bounded repository-profile fixture.
scope:
  question: Which exact result does this Frontier maintain?
  includes:
    - Rooted evidence
  excludes:
    - Unbounded discovery
maintainers:
  - maintainer:example
license:
  content: CC-BY-4.0
  code: Apache-2.0
  data: CC0-1.0
"#;

const PROFILE_B: &str = r#"
# Same value, deliberately different YAML presentation.
license: {data: "CC0-1.0", code: 'Apache-2.0', content: CC-BY-4.0}
maintainers: ["maintainer:example"]
scope:
  excludes: [Unbounded discovery]
  includes: [Rooted evidence]
  question: "Which exact result does this Frontier maintain?"
summary: "A bounded repository-profile fixture."
name: 'Example Frontier'
frontier_id: vfr_0123456789abcdef
schema: vela.frontier-profile.v1
"#;

#[test]
fn frontier_profile_v1_canonical_root_ignores_yaml_formatting() {
    let first = FrontierProfileV1::from_yaml_str(PROFILE_A).unwrap();
    let second = FrontierProfileV1::from_yaml_str(PROFILE_B).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.schema, FRONTIER_PROFILE_SCHEMA_V1);
    assert_eq!(
        first.profile_root().unwrap(),
        second.profile_root().unwrap()
    );
    assert_eq!(
        first.profile_root().unwrap(),
        "sha256:26f4cd0e61408c17b7e9f979ea8dca809a6c5ec0cbd5f22e6114ffdf68e1f1aa"
    );
}

#[test]
fn frontier_profile_v1_closed_nested_schema() {
    for invalid in [
        PROFILE_A.replace(
            "summary: A bounded repository-profile fixture.",
            "summary: A bounded repository-profile fixture.\nunknown: rejected",
        ),
        PROFILE_A.replace("  excludes:", "  unrecognized: rejected\n  excludes:"),
        PROFILE_A.replace(
            "  data: CC0-1.0",
            "  data: CC0-1.0\n  unrecognized: rejected",
        ),
    ] {
        assert!(FrontierProfileV1::from_yaml_str(&invalid).is_err());
    }
}

#[test]
fn frontier_profile_v1_rejects_duplicate_keys_aliases_and_tags() {
    let duplicate_key = PROFILE_A.replace(
        "name: Example Frontier",
        "name: Example Frontier\nname: Duplicate Frontier",
    );
    assert!(FrontierProfileV1::from_yaml_str(&duplicate_key).is_err());

    let anchored = PROFILE_A.replace(
        "name: Example Frontier\nsummary: A bounded repository-profile fixture.",
        "name: &frontier_name Example Frontier\nsummary: *frontier_name",
    );
    let error = FrontierProfileV1::from_yaml_str(&anchored).unwrap_err();
    assert!(error.contains("anchors and aliases"));

    let merge_key = PROFILE_A.replace("scope:\n", "scope:\n  <<: {}\n");
    let error = FrontierProfileV1::from_yaml_str(&merge_key).unwrap_err();
    assert!(error.contains("merge keys"));

    let tagged = PROFILE_A.replace("name: Example Frontier", "name: !vela Example Frontier");
    let error = FrontierProfileV1::from_yaml_str(&tagged).unwrap_err();
    assert!(error.contains("explicit YAML tags"));

    let core_tagged = PROFILE_A.replace("name: Example Frontier", "name: !!str Example Frontier");
    let error = FrontierProfileV1::from_yaml_str(&core_tagged).unwrap_err();
    assert!(error.contains("explicit YAML tags"));
}

#[test]
fn frontier_profile_v1_rejects_non_nfc_text() {
    let decomposed = PROFILE_A.replace("Example Frontier", "Cafe\u{301} Frontier");
    let error = FrontierProfileV1::from_yaml_str(&decomposed).unwrap_err();
    assert!(error.contains("Unicode NFC"));
}

#[test]
fn frontier_profile_v1_rejects_scope_overlap() {
    let overlapping = PROFILE_A.replace("    - Unbounded discovery", "    - Rooted evidence");
    let error = FrontierProfileV1::from_yaml_str(&overlapping).unwrap_err();
    assert!(error.contains("includes and excludes"));
}

#[test]
fn frontier_profile_v1_root_changes_on_semantic_edit() {
    let original = FrontierProfileV1::from_yaml_str(PROFILE_A).unwrap();
    let edited = FrontierProfileV1::from_yaml_str(&PROFILE_A.replace(
        "A bounded repository-profile fixture.",
        "A different bounded repository-profile fixture.",
    ))
    .unwrap();

    assert_ne!(
        original.profile_root().unwrap(),
        edited.profile_root().unwrap()
    );
}
