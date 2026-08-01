use serde_json::Value;

const ROOT_ACTION: &str = include_str!("../../../action.yml");
const INSTALLER: &str = include_str!("../../../install.sh");
const WINDOWS_INSTALLER: &str = include_str!("../../../install.ps1");
const RELEASE_WORKFLOW: &str = include_str!("../../../.github/workflows/release.yml");
const CONFORMANCE_WORKFLOW: &str = include_str!("../../../.github/workflows/conformance.yml");
const UNIX_RELEASE_SMOKE: &str = include_str!("../../../.github/release/smoke-bundle.sh");
const WINDOWS_RELEASE_SMOKE: &str = include_str!("../../../.github/release/smoke-bundle.ps1");

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

fn root_action_release_pin() -> &'static str {
    ROOT_ACTION
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix('#')
                .map(str::trim)
                .unwrap_or(trimmed)
                .strip_prefix("- uses: vela-science/vela@v")
        })
        .expect("root action must pin one exact Vela release")
}

fn is_stable_semver(version: &str) -> bool {
    let fields = version.split('.').collect::<Vec<_>>();
    fields.len() == 3
        && fields
            .iter()
            .all(|field| !field.is_empty() && field.bytes().all(|byte| byte.is_ascii_digit()))
}

fn assert_immutable_action_pins(name: &str, workflow: &str) {
    for line in workflow.lines() {
        let trimmed = line.trim();
        let Some(use_clause) = trimmed
            .strip_prefix("- uses: ")
            .or_else(|| trimmed.strip_prefix("uses: "))
        else {
            continue;
        };
        let (_, reference) = use_clause
            .split_once('@')
            .unwrap_or_else(|| panic!("{name} has malformed action use {use_clause}"));
        let reference = reference
            .split_whitespace()
            .next()
            .expect("action reference must not be empty");
        assert!(
            reference.len() == 40 && reference.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{name} action must use one immutable commit SHA: {use_clause}"
        );
    }
}

#[test]
fn root_action_is_consumer_pinned_strict_and_nonfinalizing() {
    let action = parse_action(ROOT_ACTION);
    assert!(action["inputs"].get("strict").is_none());
    assert!(action["inputs"].get("vela-version").is_none());
    assert_eq!(
        action["inputs"]
            .as_object()
            .expect("action inputs must be an object")
            .len(),
        1,
        "the public action accepts only the frontier path"
    );

    let unix_install = script_named(&action, "Install Vela on Unix");
    let windows_install = script_named(&action, "Install Vela on Windows");
    let install_steps = action["runs"]["steps"]
        .as_array()
        .expect("composite action must have steps")
        .iter();
    for step in install_steps.filter(|step| {
        matches!(
            step["name"].as_str(),
            Some("Install Vela on Unix") | Some("Install Vela on Windows")
        )
    }) {
        assert_eq!(
            step["env"]["GH_TOKEN"].as_str(),
            Some("${{ github.token }}"),
            "each installer must expose the workflow token to attestation checks"
        );
    }
    assert!(unix_install.contains("$ACTION_PATH/Cargo.toml"));
    assert!(unix_install.contains("bash \"$ACTION_PATH/install.sh\""));
    assert!(unix_install.contains("VELA_INSTALL_PREFIX=\"$install_root\""));
    assert!(unix_install.contains("\"$install_root/bin/vela\" --version"));
    assert!(windows_install.contains("install.ps1"));
    assert!(windows_install.contains("vela.exe"));

    let unix_strict = script_named(&action, "Read-only repository verification on Unix");
    let windows_strict = script_named(&action, "Read-only repository verification on Windows");
    assert!(unix_strict.contains("\"$vela_bin\" check \"$FRONTIER\" --json"));
    assert!(windows_strict.contains("check $env:FRONTIER --json"));
    assert!(!ROOT_ACTION.contains("--strict"));

    for forbidden in [
        "vela frontier",
        "snapshot_hash",
        "event_log_hash",
        "hub.constellate.science",
        "trust-anchor-file",
        ".vela/events",
    ] {
        assert!(
            !ROOT_ACTION.contains(forbidden),
            "root action retains obsolete or mutating contract `{forbidden}`"
        );
    }
    assert!(!ROOT_ACTION.contains("VELA_AUTHORITY_RECORD_ROOT"));
    assert!(!ROOT_ACTION.contains("authority trust pin"));

    assert!(!ROOT_ACTION.contains("constellate-science/vela"));
    let package_version = env!("CARGO_PKG_VERSION");
    let action_version = root_action_release_pin();
    if package_version.contains('-') {
        assert!(
            is_stable_semver(action_version),
            "prerelease source must keep the root action on one stable release, got {action_version}"
        );
    } else {
        assert_eq!(
            action_version, package_version,
            "final source and root action release must match"
        );
    }
    assert_no_finalizing_commands(&action);
}

#[test]
fn reviewed_tags_publish_provenance_labeled_cross_platform_bundles() {
    assert!(RELEASE_WORKFLOW.contains("workflow_dispatch:"));
    assert!(RELEASE_WORKFLOW.contains("tags:\n      - \"v*.*.*\""));
    assert!(RELEASE_WORKFLOW.contains("test \"v$version\" = \"$GITHUB_REF_NAME\""));
    assert!(
        RELEASE_WORKFLOW
            .contains("cargo auditable build --locked --release -p vela-cli --bin vela")
    );
    assert!(RELEASE_WORKFLOW.contains("cargo install cargo-auditable --version 0.7.5 --locked"));
    let syft_version = RELEASE_WORKFLOW
        .lines()
        .find_map(|line| line.trim().strip_prefix("syft-version: v"))
        .expect("release workflow must pin one exact Syft version");
    assert!(
        is_stable_semver(syft_version),
        "Syft must use a stable exact version, got {syft_version}"
    );
    assert!(RELEASE_WORKFLOW.contains(".github/release/check-sbom.py"));
    assert!(!RELEASE_WORKFLOW.contains("target/release/vela-signer"));
    assert!(!RELEASE_WORKFLOW.contains("target/release/vela-signer.exe"));
    for asset in [
        "vela-linux-x86_64.tar.gz",
        "vela-macos-aarch64.zip",
        "vela-windows-x86_64.zip",
    ] {
        assert!(RELEASE_WORKFLOW.contains(asset), "missing {asset}");
    }
    assert!(RELEASE_WORKFLOW.contains("test -f \"dist/$asset.sha256\""));
    assert!(RELEASE_WORKFLOW.contains("shasum -a 256 \"${{ matrix.asset }}\""));
    assert!(RELEASE_WORKFLOW.contains("Get-FileHash -Algorithm SHA256 $name"));
    assert!(RELEASE_WORKFLOW.contains("\"${name}.sha256\""));
    assert!(RELEASE_WORKFLOW.contains("actions/attest-build-provenance@"));
    assert!(RELEASE_WORKFLOW.contains("gh release create \"$GITHUB_REF_NAME\" dist/*"));
    assert!(RELEASE_WORKFLOW.contains("release_flags+=(--prerelease)"));
    assert!(RELEASE_WORKFLOW.contains("id-token: write"));
    assert!(RELEASE_WORKFLOW.contains("github.event_name == 'push'"));
    assert!(RELEASE_WORKFLOW.contains("--verify-tag"));
    assert!(RELEASE_WORKFLOW.contains("permissions:\n  contents: read"));
    assert!(RELEASE_WORKFLOW.contains("permissions:\n      contents: write"));
    for mutable_ref in [
        "actions/checkout@v",
        "dtolnay/rust-toolchain@stable",
        "Swatinem/rust-cache@v",
        "actions/upload-artifact@v",
        "actions/download-artifact@v",
    ] {
        assert!(
            !RELEASE_WORKFLOW.contains(mutable_ref),
            "release workflow retains mutable action ref {mutable_ref}"
        );
    }
}

#[test]
fn fresh_runner_smoke_precedes_publication_and_preserves_platform_checks() {
    assert!(RELEASE_WORKFLOW.contains("name: smoke-${{ matrix.asset }}"));
    assert!(RELEASE_WORKFLOW.contains(".github/release/smoke-bundle.sh"));
    assert!(RELEASE_WORKFLOW.contains(".github/release/smoke-bundle.ps1"));
    for asset in [
        "vela-linux-x86_64.tar.gz",
        "vela-macos-aarch64.zip",
        "vela-windows-x86_64.zip",
    ] {
        assert!(
            RELEASE_WORKFLOW.contains(asset),
            "smoke matrix is missing {asset}"
        );
    }
    for token in [
        "shasum -a 256 -c",
        "vela $EXPECTED_VERSION",
        "install -m 0755",
    ] {
        assert!(
            UNIX_RELEASE_SMOKE.contains(token),
            "Unix smoke is missing {token}"
        );
    }
    for token in [
        "Get-FileHash -Algorithm SHA256",
        "vela $ExpectedVersion",
        "Copy-Item -Force",
        "Remove-Item -Force",
    ] {
        assert!(
            WINDOWS_RELEASE_SMOKE.contains(token),
            "Windows smoke is missing {token}"
        );
    }
}

#[test]
fn every_hosted_action_is_pinned_by_commit() {
    for (name, workflow) in [
        ("conformance", CONFORMANCE_WORKFLOW),
        ("release", RELEASE_WORKFLOW),
    ] {
        assert_immutable_action_pins(name, workflow);
    }
}

#[test]
fn installer_points_to_the_nonfinalizing_task_first_path() {
    assert!(!INSTALLER.contains("vela-signer"));
    assert!(!INSTALLER.contains("science.vela.signer.policy"));
    assert!(!WINDOWS_INSTALLER.contains("vela-signer.exe"));
    assert!(WINDOWS_INSTALLER.contains("Get-FileHash -Algorithm SHA256"));
    assert!(INSTALLER.contains("VELA_EXPECTED_SHA256"));
    assert!(WINDOWS_INSTALLER.contains("VELA_EXPECTED_SHA256"));
    assert!(INSTALLER.contains("differs from the ecosystem-lock SHA-256"));
    assert!(WINDOWS_INSTALLER.contains("differs from the ecosystem-lock SHA-256"));
    assert!(INSTALLER.contains("vela check . --json"));
    assert!(INSTALLER.contains("vela next . --json"));
    assert!(INSTALLER.contains("vela start <target>"));
    assert!(INSTALLER.contains("vela submit --frontier . --claim <claim>"));
    assert!(!INSTALLER.contains("--attempt"));
    assert!(!INSTALLER.contains("vela work <target>"));
    assert!(INSTALLER.contains("docs/PRODUCER_QUICKSTART.md"));
    for forbidden in ["vela finding add", "--apply", "vela sign", "vela accept"] {
        assert!(
            !INSTALLER.contains(forbidden),
            "installer must not recommend `{forbidden}`"
        );
    }
}
