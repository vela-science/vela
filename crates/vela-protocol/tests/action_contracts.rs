use serde_json::Value;

const ROOT_ACTION: &str = include_str!("../../../action.yml");
const INSTALLER: &str = include_str!("../../../install.sh");
const RELEASE_WORKFLOW: &str = include_str!("../../../.github/workflows/release.yml");
const CONFORMANCE_WORKFLOW: &str = include_str!("../../../.github/workflows/conformance.yml");

fn parse_yaml(source: &str) -> Value {
    serde_yaml_ng::from_str(source).expect("source must be valid YAML")
}

fn steps(container: &Value) -> &[Value] {
    container["steps"]
        .as_array()
        .expect("action or workflow job must have steps")
}

fn step_named<'a>(container: &'a Value, name: &str) -> &'a Value {
    steps(container)
        .iter()
        .find(|step| step["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing step {name}"))
}

fn script_named<'a>(container: &'a Value, name: &str) -> &'a str {
    step_named(container, name)["run"]
        .as_str()
        .unwrap_or_else(|| panic!("step {name} must run a script"))
}

fn workflow_job<'a>(workflow: &'a Value, name: &str) -> &'a Value {
    workflow["jobs"]
        .get(name)
        .unwrap_or_else(|| panic!("workflow is missing job {name}"))
}

fn matrix_assets(job: &Value) -> Vec<&str> {
    let mut assets = job["strategy"]["matrix"]["include"]
        .as_array()
        .expect("release job must have an include matrix")
        .iter()
        .map(|entry| {
            entry["asset"]
                .as_str()
                .expect("release matrix entry must name an asset")
        })
        .collect::<Vec<_>>();
    assets.sort_unstable();
    assets
}

fn needs(job: &Value) -> Vec<&str> {
    match &job["needs"] {
        Value::String(value) => vec![value],
        Value::Array(values) => values
            .iter()
            .map(|value| value.as_str().expect("job dependency must be a string"))
            .collect(),
        _ => panic!("release job must declare its dependencies"),
    }
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
    for script in steps(&action["runs"])
        .iter()
        .filter_map(|step| step["run"].as_str())
    {
        for command in forbidden {
            assert!(
                !script.contains(command),
                "producer action must not contain finalizing command `{command}`"
            );
        }
    }
}

fn is_stable_semver(version: &str) -> bool {
    let fields = version.split('.').collect::<Vec<_>>();
    fields.len() == 3
        && fields
            .iter()
            .all(|field| !field.is_empty() && field.bytes().all(|byte| byte.is_ascii_digit()))
}

fn assert_immutable_action_pins(name: &str, workflow: &str) {
    let workflow = parse_yaml(workflow);
    for job in workflow["jobs"]
        .as_object()
        .expect("workflow jobs must be an object")
        .values()
    {
        for step in steps(job) {
            let Some(use_clause) = step["uses"].as_str() else {
                continue;
            };
            let (_, reference) = use_clause
                .split_once('@')
                .unwrap_or_else(|| panic!("{name} has malformed action use {use_clause}"));
            assert!(
                reference.len() == 40 && reference.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "{name} action must use one immutable commit SHA: {use_clause}"
            );
            if use_clause.starts_with("actions/checkout@") {
                assert_eq!(
                    step["with"]["persist-credentials"].as_bool(),
                    Some(false),
                    "{name} checkout must not persist its credential"
                );
            }
        }
    }
}

#[test]
fn root_action_is_read_only_and_nonfinalizing() {
    let action = parse_yaml(ROOT_ACTION);
    assert_eq!(action["runs"]["using"].as_str(), Some("composite"));
    assert!(action["inputs"].get("strict").is_none());
    assert!(action["inputs"].get("vela-version").is_none());
    assert_eq!(
        action["inputs"]
            .as_object()
            .expect("action inputs must be an object")
            .len(),
        1,
        "the public action accepts only the repository path"
    );

    let install = step_named(&action["runs"], "Install Vela");
    assert_eq!(
        install["env"]["GH_TOKEN"].as_str(),
        Some("${{ github.token }}"),
        "the installer must expose the workflow token to attestation checks"
    );
    assert!(script_named(&action["runs"], "Require a supported runner").contains("Linux|macOS"));
    let strict = script_named(&action["runs"], "Read-only repository verification");
    /* Shape, not verb. Pinning the literal subcommand here is what let the
    `check` → `replay` rename ship with the Action still calling `check`:
    this assertion passed on the stale string. Whether the verb exists is
    proved by running the binary, in vela-cli/tests/action_invocation.rs. */
    assert!(
        strict.contains("\"$vela_bin\""),
        "the step must invoke the pinned binary"
    );
    assert!(
        strict.contains("\"$FRONTIER\" --json"),
        "the step must verify the consumer's repository as JSON"
    );

    /* The two shape checks every repository owes and none of them owns. They are
    named here because four repositories consume this file instead of carrying
    four copies: deleting a step from it silently ungates all four at once. */
    assert!(
        script_named(
            &action["runs"],
            "Committed source lock matches its declaration"
        )
        .contains("vela-source-lock --check")
    );
    assert!(
        script_named(&action["runs"], "Repository shape")
            .contains("conformance/repository_lint.py")
    );

    /* The action now uses a hosted action of its own. Consumers pin this file
    by SHA and cannot see what it resolves at run time, so an unpinned use here
    would make their pin promise less than it says. */
    for step in steps(&action["runs"]) {
        let Some(use_clause) = step["uses"].as_str() else {
            continue;
        };
        let (_, reference) = use_clause
            .split_once('@')
            .unwrap_or_else(|| panic!("the public action has malformed action use {use_clause}"));
        assert!(
            reference.len() == 40 && reference.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "the public action must use one immutable commit SHA: {use_clause}"
        );
    }

    assert_no_finalizing_commands(&action);
}

#[test]
fn reviewed_tags_publish_provenance_labeled_supported_bundles() {
    let workflow = parse_yaml(RELEASE_WORKFLOW);
    let triggers = &workflow["on"];
    assert!(triggers.get("workflow_dispatch").is_some());
    assert_eq!(
        triggers["push"]["tags"]
            .as_array()
            .expect("release push trigger must declare tag patterns")
            .as_slice(),
        &[Value::String("v*.*.*".into())]
    );
    let build = workflow_job(&workflow, "build");
    let publish = workflow_job(&workflow, "publish");
    assert_eq!(publish["if"].as_str(), Some("github.event_name == 'push'"));
    assert_eq!(workflow["permissions"]["contents"].as_str(), Some("read"));
    assert_eq!(build["permissions"]["id-token"].as_str(), Some("write"));
    assert_eq!(build["permissions"]["attestations"].as_str(), Some("write"));
    assert_eq!(publish["permissions"]["contents"].as_str(), Some("write"));

    let build_script = script_named(build, "Build auditable release binary");
    /* This used to name the cargo-auditable version, which is a second copy of
    a number the workflow already carries: a correct bump reddened the test for
    a reason that had nothing to do with the release contract. What the release
    depends on is the shape the Syft pin below is held to — one exact stable
    version, installed from the locked index. */
    let auditable_pin = build_script
        .split_once("cargo install cargo-auditable --version ")
        .map(|(_, rest)| rest)
        .expect("the build must install cargo-auditable at one pinned version");
    let (auditable_version, install_flags) = auditable_pin
        .split_once(char::is_whitespace)
        .expect("the cargo-auditable pin must be followed by its install flags");
    assert!(
        is_stable_semver(auditable_version),
        "cargo-auditable must pin a stable exact version, got {auditable_version}"
    );
    assert!(
        install_flags.starts_with("--locked"),
        "cargo-auditable must install from the locked index"
    );
    assert!(
        build_script.contains("cargo auditable build --locked --release -p vela-cli --bin vela")
    );
    assert!(
        script_named(workflow_job(&workflow, "metadata"), "Bind release identity")
            .contains("test \"v$version\" = \"$GITHUB_REF_NAME\"")
    );

    let sbom = step_named(build, "Generate SPDX SBOM");
    assert_eq!(sbom["with"]["format"].as_str(), Some("spdx-json"));
    assert_eq!(sbom["with"]["upload-artifact"].as_bool(), Some(false));
    assert_eq!(sbom["with"]["upload-release-assets"].as_bool(), Some(false));
    let syft_version = sbom["with"]["syft-version"]
        .as_str()
        .and_then(|version| version.strip_prefix('v'))
        .expect("SBOM generation must pin one exact Syft version");
    assert!(
        is_stable_semver(syft_version),
        "Syft must use a stable exact version, got {syft_version}"
    );
    assert!(
        script_named(build, "Verify SBOM includes Vela").contains(".github/release/check-sbom.py")
    );

    let expected_assets = ["vela-linux-x86_64.tar.gz", "vela-macos-aarch64.zip"];
    assert_eq!(matrix_assets(build), expected_assets);
    assert!(steps(build).iter().any(|step| {
        step["uses"]
            .as_str()
            .is_some_and(|value| value.starts_with("actions/attest-build-provenance@"))
    }));
    assert!(script_named(build, "Write checksums").contains("shasum -a 256"));

    let publish_script = script_named(publish, "Publish immutable GitHub release");
    for asset in expected_assets {
        assert!(
            publish_script.contains(asset),
            "publication omitted {asset}"
        );
    }
    assert!(publish_script.contains("$asset.sha256"));
    assert!(publish_script.contains("$asset.spdx.json"));
    assert!(publish_script.contains("$asset.spdx.json.sha256"));
    assert!(publish_script.contains("gh release create"));
    assert!(publish_script.contains("--verify-tag"));
}

#[test]
fn fresh_runner_smoke_precedes_publication_for_supported_platforms() {
    let workflow = parse_yaml(RELEASE_WORKFLOW);
    let build = workflow_job(&workflow, "build");
    let smoke = workflow_job(&workflow, "smoke");
    let publish = workflow_job(&workflow, "publish");
    assert_eq!(matrix_assets(smoke), matrix_assets(build));
    assert!(needs(smoke).contains(&"build"));
    assert!(needs(publish).contains(&"smoke"));
    assert!(
        script_named(smoke, "Smoke release bundle").contains(".github/release/smoke-bundle.sh")
    );
}

/// The one step that installs the Python reader, wherever it is declared.
fn locked_reader_step<'a>(container: &'a Value, name: &str) -> &'a Value {
    let mut found = steps(container).iter().filter(|step| {
        step["uses"]
            .as_str()
            .is_some_and(|clause| clause.starts_with("astral-sh/setup-uv@"))
    });
    let step = found
        .next()
        .unwrap_or_else(|| panic!("{name} must install the locked reader"));
    assert!(
        found.next().is_none(),
        "{name} installs the reader twice; one of them will be the stale one"
    );
    step
}

#[test]
fn the_action_and_conformance_install_the_same_locked_reader() {
    // Both run this repository's own `--locked` projects, so they must agree on
    // the interpreter and the resolver that read those locks. A composite
    // action cannot borrow a workflow's inputs, so the pin is written twice by
    // necessity — which is exactly the kind of second copy that goes stale on
    // the next bump. It is bound here instead of trusted.
    let action = parse_yaml(ROOT_ACTION);
    let workflow = parse_yaml(CONFORMANCE_WORKFLOW);
    let by_action = locked_reader_step(&action["runs"], "the root action");
    let by_conformance = locked_reader_step(workflow_job(&workflow, "rust"), "conformance");

    assert_eq!(
        by_action["uses"], by_conformance["uses"],
        "the action and conformance disagree about which reader they install"
    );
    for field in ["version", "python-version"] {
        assert_eq!(
            by_action["with"][field], by_conformance["with"][field],
            "the action and conformance disagree about the reader's {field}"
        );
        assert!(
            by_action["with"][field].as_str().is_some(),
            "the reader's {field} must be pinned, not left to the day the job ran"
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
fn installers_verify_release_bytes_and_do_not_ship_a_signer() {
    assert!(!INSTALLER.contains("vela-signer"));
    assert!(!INSTALLER.contains("science.vela.signer.policy"));
    assert!(INSTALLER.contains("VELA_EXPECTED_SHA256"));
}
