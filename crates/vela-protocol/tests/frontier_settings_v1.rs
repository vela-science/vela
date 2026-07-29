use vela_protocol::frontier_settings::{
    FRONTIER_SETTINGS_SCHEMA, FrontierGitPush, FrontierSettingsV1,
};

#[test]
fn frontier_settings_v1_accepts_only_the_operational_allowlist() {
    let parsed = FrontierSettingsV1::from_toml(
        r#"
schema = "vela.frontier-settings.v1"

[publish]
git_push = "off"

[work]
lease_ttl_seconds = 43200

"#,
    )
    .unwrap();
    assert_eq!(parsed.schema, FRONTIER_SETTINGS_SCHEMA);
    assert_eq!(parsed.publish.unwrap().git_push, FrontierGitPush::Off);
    assert_eq!(parsed.work.unwrap().lease_ttl_seconds, 43_200);
}

#[test]
fn frontier_settings_v1_rejects_unknown_authority_and_custody_surfaces() {
    for document in [
        "schema = \"vela.frontier-settings.v1\"\ntoken = \"secret\"\n",
        "schema = \"vela.frontier-settings.v1\"\n[keys]\npath = \"private.key\"\n",
        "schema = \"vela.frontier-settings.v1\"\n[network]\nendpoint = \"https://example.test\"\n",
        "schema = \"vela.frontier-settings.v1\"\n[policy]\nauto_accept = true\n",
        "schema = \"vela.frontier-settings.v1\"\n[actor]\nid = \"reviewer:test\"\n",
        "schema = \"vela.frontier-settings.v1\"\n[verifier]\ncommand = \"true\"\n",
        "schema = \"vela.frontier-settings.v1\"\n[dependencies]\nref = \"main\"\n",
        "schema = \"vela.frontier-settings.v1\"\n[hooks]\npost_save = \"true\"\n",
    ] {
        assert!(
            FrontierSettingsV1::from_toml(document).is_err(),
            "accepted forbidden document:\n{document}"
        );
    }
}

#[test]
fn frontier_settings_v1_fails_closed_on_unknown_or_widening_values() {
    for document in [
        "schema = \"vela.frontier-settings.v1\"\nunknown = true\n",
        "schema = \"vela.frontier-settings.v1\"\n[publish]\ngit_push = \"auto\"\n",
        "schema = \"vela.frontier-settings.v1\"\n[work]\nlease_ttl_seconds = 0\n",
        "schema = \"vela.frontier-settings.v1\"\n[mcp]\nprofile = \"authority\"\n",
        "schema = \"wrong\"\n",
    ] {
        assert!(
            FrontierSettingsV1::from_toml(document).is_err(),
            "accepted invalid document:\n{document}"
        );
    }
}
