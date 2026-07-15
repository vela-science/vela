use serde_json::Value;

const ROOT_ACTION: &str = include_str!("../../../action.yml");
const LOCAL_ACTION: &str = include_str!("../../../.github/actions/vela-check/action.yml");
const RELEASE_WORKFLOW: &str = include_str!("../../../.github/workflows/release.yml");

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
    assert_eq!(action["inputs"]["strict"]["default"], "true");

    let install = script_named(
        &action,
        "Install vela (pinned release binary, no from-source build)",
    );
    assert!(install.contains("lock_version=\"$(awk '/^vela_version:/{print $2}' \"$lock\")\""));
    assert!(install.contains("requested=\"${VELA_VERSION_INPUT#v}\""));
    assert!(install.contains("requested Vela $requested does not match $lock's vela_version"));
    assert!(install.contains("release=\"v$lock_version\""));
    assert!(
        install.contains("https://raw.githubusercontent.com/vela-science/vela/$release/install.sh")
    );
    assert!(install.contains("installed Vela $installed does not match $lock's vela_version"));

    let strict = script_named(&action, "Strict trust gate");
    assert!(strict.contains("case \"$STRICT\" in"));
    assert!(strict.contains("true)"));
    assert!(strict.contains("false)"));
    assert!(strict.contains("vela check \"$FRONTIER\" --strict"));
    assert!(strict.contains("strict must be exactly"));

    assert!(!ROOT_ACTION.contains("constellate-science/vela"));
    assert!(!ROOT_ACTION.contains("/main/install.sh"));
    assert!(ROOT_ACTION.contains(&format!(
        "uses: vela-science/vela@v{}",
        env!("CARGO_PKG_VERSION")
    )));
    assert_no_finalizing_commands(&action);
}

#[test]
fn local_auto_action_requires_one_lock_version_and_blocks_strict_by_default() {
    let action = parse_action(LOCAL_ACTION);
    assert_eq!(action["inputs"]["strict"]["default"], "true");

    let install = script_named(
        &action,
        "Install vela (pinned binary, no from-source build)",
    );
    assert!(install.contains("mapfile -t frontiers"));
    assert!(install.contains("candidate=\"$(awk '/^vela_version:/{print $2}' \"$lock\")\""));
    assert!(install.contains("selected frontier locks require multiple Vela releases"));
    assert!(
        install.contains("requested Vela $requested does not match selected locks' vela_version")
    );
    assert!(install.contains("release=\"v$lock_version\""));
    assert!(
        install.contains("https://raw.githubusercontent.com/vela-science/vela/$release/install.sh")
    );
    assert!(
        install.contains("installed Vela $installed does not match selected locks' vela_version")
    );

    let gate = script_named(
        &action,
        "re-derive (reproduce + check + hash-parity, per frontier)",
    );
    assert!(gate.contains("case \"$STRICT\" in"));
    assert!(gate.contains("if [ \"$STRICT\" = \"true\" ]; then"));
    assert!(gate.contains("strict proof-readiness or state-integrity debt is blocking"));
    assert!(gate.contains("strict: false explicitly acknowledges owner key-custody debt"));

    assert!(!LOCAL_ACTION.contains("constellate-science/vela"));
    assert!(!LOCAL_ACTION.contains("/main/install.sh"));
    assert_no_finalizing_commands(&action);
}

#[test]
fn release_refuses_tag_version_drift_and_builds_the_lockfile() {
    assert!(RELEASE_WORKFLOW.contains("Verify tag matches workspace version"));
    assert!(RELEASE_WORKFLOW.contains("expected_tag=\"v${workspace_version}\""));
    assert!(RELEASE_WORKFLOW.contains("[ \"$GITHUB_REF_NAME\" != \"$expected_tag\" ]"));
    assert!(RELEASE_WORKFLOW.contains("cargo build --locked --release --bin vela"));
    assert!(!RELEASE_WORKFLOW.contains("cargo build --release --bin vela"));
}
