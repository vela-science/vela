use std::process::Command;

fn vela_bin() -> &'static str {
    env!("CARGO_BIN_EXE_vela")
}

fn run(frontier: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(vela_bin())
        .args(args)
        .env("HOME", frontier.join("home"))
        .output()
        .expect("run vela")
}

#[test]
fn config_reads_closed_v1_frontier_settings() {
    let temp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(temp.path().join(".vela")).unwrap();
    std::fs::write(
        temp.path().join(".vela/settings.toml"),
        r#"schema = "vela.frontier-settings.v1"

[work]
lease_ttl_seconds = 43200
"#,
    )
    .unwrap();

    let output = run(
        temp.path(),
        &[
            "config",
            "get",
            "work.lease_ttl_seconds",
            "--frontier",
            temp.path().to_str().unwrap(),
            "--json",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["value"], "43200");
    assert_eq!(payload["origin"], "frontier");
}

#[test]
fn config_rejects_invalid_v1_settings_without_using_legacy_values() {
    let temp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(temp.path().join(".vela")).unwrap();
    std::fs::write(
        temp.path().join(".vela/settings.toml"),
        "schema = \"vela.frontier-settings.v1\"\n[keys]\npath = \"private.key\"\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join(".vela/config.toml"),
        "[work]\nlease_ttl_seconds = 5\n",
    )
    .unwrap();

    let output = run(
        temp.path(),
        &[
            "config",
            "get",
            "work.lease_ttl_seconds",
            "--frontier",
            temp.path().to_str().unwrap(),
            "--json",
        ],
    );
    assert!(!output.status.success(), "{output:?}");
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["ok"], false);
    assert!(
        payload["error"]["message"]
            .as_str()
            .unwrap()
            .contains("invalid Frontier settings")
    );
}

#[test]
fn config_keeps_reading_v01_legacy_frontier_preferences() {
    let temp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(temp.path().join(".vela")).unwrap();
    std::fs::write(
        temp.path().join(".vela/config.toml"),
        "[work]\nlease_ttl_seconds = 7200\n",
    )
    .unwrap();

    let output = run(
        temp.path(),
        &[
            "config",
            "get",
            "work.lease_ttl_seconds",
            "--frontier",
            temp.path().to_str().unwrap(),
            "--json",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["value"], "7200");
    assert_eq!(payload["origin"], "frontier");
}
