use serde_json::Value;

const ROOT_ACTION: &str = include_str!("../../../action.yml");
const INSTALLER: &str = include_str!("../../../install.sh");

fn parse_action(source: &str) -> Value {
    serde_yaml::from_str(source).expect("action source must be valid YAML")
}

fn run_scripts(action: &Value) -> Vec<&str> {
    action["runs"]["steps"]
        .as_array()
        .expect("composite action must have steps")
        .iter()
        .filter_map(|step| step["run"].as_str())
        .collect()
}

fn script_named<'a>(action: &'a Value, name: &str) -> &'a str {
    action["runs"]["steps"]
        .as_array()
        .expect("composite action must have steps")
        .iter()
        .find(|step| step["name"].as_str() == Some(name))
        .and_then(|step| step["run"].as_str())
        .unwrap_or_else(|| panic!("missing run script for step {name}"))
}

fn assert_no_finalizing_commands(action: &Value) {
    let forbidden = [
        "vela sign",
        "vela accept",
        "vela review",
        "vela proposals accept",
        "vela proposals reject",
        "vela policy accept",
    ];
    for script in run_scripts(action) {
        for command in forbidden {
            assert!(
                !script.contains(command),
                "producer action must not contain finalizing command `{command}`"
            );
        }
    }
}

#[test]
fn root_action_is_lock_pinned_strict_and_nonfinalizing() {
    let action = parse_action(ROOT_ACTION);
    assert!(action["inputs"].get("strict").is_none());
    assert_eq!(action["inputs"]["vela-version"]["required"], true);

    let install = script_named(
        &action,
        "Install vela (pinned release binary, no from-source build)",
    );
    assert!(install.contains("lock_version=\"$(awk '/^vela_version:/{print $2}' \"$lock\")\""));
    assert!(install.contains("requested=\"${VELA_VERSION_INPUT#v}\""));
    assert!(install.contains("vela-version is required"));
    assert!(install.contains("requested Vela $requested does not match $lock's vela_version"));
    assert!(install.contains("release=\"v$lock_version\""));
    assert!(
        install.contains("https://raw.githubusercontent.com/vela-science/vela/$release/install.sh")
    );
    assert!(install.contains("installed Vela $installed does not match $lock's vela_version"));

    let strict = script_named(&action, "Strict trust gate");
    assert!(strict.contains("vela check \"$FRONTIER\" --strict"));
    assert!(!strict.contains("STRICT"));
    assert!(!strict.contains("::notice::"));

    assert!(!ROOT_ACTION.contains("constellate-science/vela"));
    assert!(!ROOT_ACTION.contains("/main/install.sh"));
    assert!(ROOT_ACTION.contains(&format!(
        "uses: vela-science/vela@v{}",
        env!("CARGO_PKG_VERSION")
    )));
    assert_no_finalizing_commands(&action);
}

#[test]
fn prelaunch_tags_do_not_publish_a_github_release() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert!(
        !workspace.join(".github/workflows/release.yml").exists(),
        "prelaunch tags must not trigger a GitHub Release"
    );
}

#[test]
fn installer_points_to_the_nonfinalizing_task_first_path() {
    assert!(INSTALLER.contains("vela check . --strict --json"));
    assert!(INSTALLER.contains("vela next . --json"));
    assert!(INSTALLER.contains("docs/PRODUCER_QUICKSTART.md"));
    for forbidden in ["vela finding add", "--apply", "vela sign", "vela accept"] {
        assert!(
            !INSTALLER.contains(forbidden),
            "installer must not recommend `{forbidden}`"
        );
    }
}
