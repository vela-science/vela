//! What a consumer meets before the binary exists: the installer, the release
//! workflow, and the manifest between them.
//!
//! This was `action_contracts.rs`, and its largest test walked the composite
//! action in `action.yml` asserting it stayed read-only and non-finalizing.
//! The action is gone — every consumer of it was archived and pinned to an
//! immutable commit — and what is left here never depended on it.

use serde_json::Value;

const INSTALLER: &str = include_str!("../../../install.sh");
const CITATION: &str = include_str!("../../../CITATION.cff");
const RELEASE_WORKFLOW: &str = include_str!("../../../.github/workflows/release.yml");
const CONFORMANCE_WORKFLOW: &str = include_str!("../../../.github/workflows/conformance.yml");
/* The release semantics moved out of the workflow and into an entry point a
clean checkout can run with no CI provider. This test moved with them: what it
protects is the order and the pins, not which file happens to hold them. */
const RELEASE_SCRIPT: &str = include_str!("../../../scripts/release.sh");
const SBOM_CANONICALIZER: &str = include_str!("../../../.github/release/check-sbom.py");
const SIGN_PUBLISHED_RELEASE: &str = include_str!("../../../scripts/sign-published-release.sh");

fn parse_yaml(source: &str) -> Value {
    serde_saphyr::from_str(source).expect("source must be valid YAML")
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

/// Read one `NAME="value"` assignment out of a shell script.
///
/// The entry point declares its pins as constants at the top rather than
/// burying them in the command that uses them, so this reads the declaration.
fn shell_constant<'a>(source: &'a str, name: &str) -> &'a str {
    let needle = format!("\n{name}=\"");
    let (_, rest) = source
        .split_once(&needle)
        .unwrap_or_else(|| panic!("scripts/release.sh declares no {name}"));
    rest.split_once('"')
        .unwrap_or_else(|| panic!("{name} is not a closed string"))
        .0
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

    /* The build job runs one entry point. Both jobs call it: the version the
    release is identified by and the version it is built from are read by the
    same code, which is the property that used to depend on two `Cargo.toml`
    readers agreeing. */
    assert!(
        script_named(build, "Build the release bundle").contains("scripts/release.sh"),
        "the build must call the provider-neutral entry point"
    );
    let metadata_script =
        script_named(workflow_job(&workflow, "metadata"), "Bind release identity");
    assert!(metadata_script.contains("scripts/release.sh --print-version"));
    assert!(metadata_script.contains("test \"v$version\" = \"$GITHUB_REF_NAME\""));

    /* What the release depends on, now asserted where it lives. `--locked` on
    both the toolchain install and the build; one exact stable version for
    cargo-auditable and for Syft; the SBOM content check, the checksums and the
    bundle smoke test all reached from the same script. */
    let auditable_version = shell_constant(RELEASE_SCRIPT, "CARGO_AUDITABLE_VERSION");
    assert!(
        is_stable_semver(auditable_version),
        "cargo-auditable must pin a stable exact version, got {auditable_version}"
    );
    assert!(
        RELEASE_SCRIPT.contains(
            "cargo install cargo-auditable --version \"$CARGO_AUDITABLE_VERSION\" --locked"
        )
    );
    assert!(
        RELEASE_SCRIPT.contains("cargo auditable build --locked --release -p vela-cli --bin vela")
    );
    assert!(RELEASE_SCRIPT.contains(".github/release/check-sbom.py"));
    assert!(RELEASE_SCRIPT.contains("shasum -a 256"));
    assert!(RELEASE_SCRIPT.contains(".github/release/smoke-bundle.sh"));

    /* Syft's version is written twice by necessity — the workflow decides which
    binary is downloaded and the script decides which one it will accept — so
    the two copies are held equal here rather than trusted to be bumped
    together. The marketplace action no longer decides the scan path, the
    format or the output file; those are release semantics and live in the
    entry point. */
    let syft_version = shell_constant(RELEASE_SCRIPT, "SYFT_VERSION");
    assert!(
        is_stable_semver(syft_version),
        "Syft must use a stable exact version, got {syft_version}"
    );
    let downloaded_syft = steps(build)
        .iter()
        .find(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|clause| clause.starts_with("anchore/sbom-action/download-syft@"))
        })
        .expect("the build must install the pinned Syft");
    assert_eq!(
        downloaded_syft["with"]["syft-version"].as_str(),
        Some(format!("v{syft_version}").as_str()),
        "the workflow downloads a Syft the entry point will refuse"
    );
    assert!(RELEASE_SCRIPT.contains("spdx-json="));

    let expected_assets = ["vela-linux-x86_64.tar.gz", "vela-macos-aarch64.zip"];
    assert_eq!(matrix_assets(build), expected_assets);
    assert!(steps(build).iter().any(|step| {
        step["uses"]
            .as_str()
            .is_some_and(|value| value.starts_with("actions/attest-build-provenance@"))
    }));

    let publish_script = script_named(publish, "Publish the release as a draft for signing");
    for asset in expected_assets {
        assert!(
            publish_script.contains(asset),
            "publication omitted {asset}"
        );
    }
    assert!(publish_script.contains("$asset.sha256"));
    assert!(publish_script.contains("$asset.spdx.json"));
    assert!(publish_script.contains("$asset.spdx.json.sha256"));
    assert!(publish_script.contains("$asset.release-manifest.json"));
    assert!(publish_script.contains("gh release create"));
    assert!(publish_script.contains("--verify-tag"));

    // A draft, and it has to stay one until an operator signs it.
    //
    // A published release in this repository is immutable, which is right for a
    // scientific artifact and refuses new assets outright. The manifest is built
    // here and deliberately not signed here, so publishing straight away closed
    // the door on the signature — observed as `422 Cannot upload assets to an
    // immutable release` on v0.968.0. Dropping `--draft` would restore that,
    // silently, and the release would look fine until someone tried to sign it.
    assert!(
        publish_script.contains("--draft"),
        "the release must be published as a draft so it can be signed before it becomes immutable"
    );
}

/// The release manifest is not the scientific authority record.
///
/// `docs/SIGNING.md` scopes the repository-authority key to attesting that a
/// principal, authorization, semantic action, read-set recheck and canonical
/// write matched. Publishing a binary is none of those. The entry point can
/// sign a manifest, and this holds it to a separate identity and to a schema id
/// that does not collide with `vela.observatory-release-manifest`, which is an
/// unrelated `vela-web` read projection.
#[test]
fn the_release_manifest_is_distribution_evidence_not_repository_authority() {
    let schema = shell_constant(RELEASE_SCRIPT, "MANIFEST_SCHEMA");
    assert!(
        schema.starts_with("vela.") && schema.ends_with(".v1"),
        "the manifest must carry one versioned Vela schema id, got {schema}"
    );
    assert!(
        !schema.contains("observatory-release-manifest"),
        "the release manifest must not claim the Observatory projection's schema id"
    );

    assert_eq!(
        shell_constant(RELEASE_SCRIPT, "SIGNATURE_NAMESPACE"),
        "vela-release"
    );
    /* `-U` signs through ssh-agent from a public key, so the entry point never
    reads private key material. Same custody rule the CLI uses. */
    assert!(RELEASE_SCRIPT.contains("ssh-keygen -Y sign -f \"$SIGN_KEY\" -U -n"));
    assert!(
        RELEASE_SCRIPT.contains("*repository_authority*|*repository-authority*"),
        "the entry point must refuse to sign a release with the repository-authority key"
    );
}

#[test]
fn release_bytes_are_compared_before_the_manifest_is_emitted() {
    assert!(RELEASE_SCRIPT.contains("SOURCE_DATE_EPOCH"));
    assert!(RELEASE_SCRIPT.contains("--remap-path-prefix=$target_dir=$REMAP_TARGET_PREFIX"));
    assert!(RELEASE_SCRIPT.contains("build_release \"$BUILD_ONE\""));
    assert!(RELEASE_SCRIPT.contains("build_release \"$BUILD_TWO\""));
    assert!(RELEASE_SCRIPT.contains("cmp \"$BUILD_ONE/release/vela\" \"$BUILD_TWO/release/vela\""));

    let archiver = ".github/release/create-deterministic-archive.py";
    assert_eq!(
        RELEASE_SCRIPT.matches(archiver).count(),
        2,
        "the release must independently archive the staged tree twice"
    );
    assert!(RELEASE_SCRIPT.contains("cmp \"$ARCHIVE\" \"$ARCHIVE_CHECK\""));
    assert!(RELEASE_SCRIPT.contains("--binary-build-count 2"));
    assert!(RELEASE_SCRIPT.contains("--archive-build-count 2"));
}

#[test]
fn staged_release_binary_remaps_and_refuses_builder_private_paths() {
    for remap in [
        "--remap-path-prefix=$target_dir=$REMAP_TARGET_PREFIX",
        "--remap-path-prefix=$ROOT=$REMAP_SOURCE_PREFIX",
        "--remap-path-prefix=$CARGO_HOME_RESOLVED=$REMAP_CARGO_HOME_PREFIX",
        "--remap-path-prefix=$ACCOUNT_HOME_RESOLVED=$REMAP_ACCOUNT_HOME_PREFIX",
    ] {
        assert!(
            RELEASE_SCRIPT.contains(remap),
            "release build is missing private-path remap {remap}"
        );
    }

    assert!(RELEASE_SCRIPT.contains("LC_ALL=C grep -aFq -- \"$path\" \"$artifact\""));
    for refusal in [
        "refuse_private_path_bytes \"$STAGE/vela\" \"$ROOT\"",
        "refuse_private_path_bytes \"$STAGE/vela\" \"$BUILD_ONE\"",
        "refuse_private_path_bytes \"$STAGE/vela\" \"$BUILD_TWO\"",
        "refuse_private_path_bytes \"$STAGE/vela\" \"$CARGO_HOME_RESOLVED\"",
        "refuse_private_path_bytes \"$STAGE/vela\" \"$ACCOUNT_HOME_RESOLVED\"",
    ] {
        assert!(
            RELEASE_SCRIPT.contains(refusal),
            "release staging is missing private-path refusal {refusal}"
        );
    }
}

#[test]
fn public_sbom_is_deterministic_and_refuses_private_paths() {
    assert_eq!(RELEASE_SCRIPT.matches("\"$SYFT\" scan").count(), 2);
    assert_eq!(
        RELEASE_SCRIPT
            .matches("canonicalize_sbom \"$SBOM_RAW_")
            .count(),
        2
    );
    assert!(RELEASE_SCRIPT.contains("cmp \"$SBOM\" \"$SBOM_CHECK\""));
    assert!(RELEASE_SCRIPT.contains("SBOM_CREATED=\"$(\"$PYTHON\" - \"$SOURCE_DATE_EPOCH\""));
    for field in [
        "name",
        "documentNamespace",
        "created",
        "root-name",
        "root-id",
    ] {
        assert!(
            SBOM_CANONICALIZER.contains(field),
            "SBOM canonicalizer does not close {field}"
        );
    }
    for refusal in [
        "refuse_private_path_bytes \"$SBOM\" \"$STAGE\"",
        "refuse_private_path_bytes \"$SBOM\" \"$ROOT\"",
        "refuse_private_path_bytes \"$SBOM\" \"$CARGO_HOME_RESOLVED\"",
        "refuse_private_path_bytes \"$SBOM\" \"$ACCOUNT_HOME_RESOLVED\"",
    ] {
        assert!(
            RELEASE_SCRIPT.contains(refusal),
            "release staging is missing SBOM private-path refusal {refusal}"
        );
    }
}

#[test]
fn signing_refuses_a_partial_or_mismatched_draft() {
    for manifest in [
        "vela-linux-x86_64.tar.gz.release-manifest.json",
        "vela-macos-aarch64.zip.release-manifest.json",
    ] {
        assert!(
            SIGN_PUBLISHED_RELEASE.contains(manifest),
            "the signing gate must require {manifest}"
        );
    }
    assert!(SIGN_PUBLISHED_RELEASE.contains("expected exactly ${#EXPECTED_MANIFESTS[@]}"));
    assert!(SIGN_PUBLISHED_RELEASE.contains("observed_assets != expected_assets"));
    assert!(SIGN_PUBLISHED_RELEASE.contains("binary_builds_compared"));
    assert!(SIGN_PUBLISHED_RELEASE.contains("archive_builds_compared"));
    assert!(SIGN_PUBLISHED_RELEASE.contains("[ \"$manifest_commit\" = \"$tag_commit\" ]"));
    assert!(SIGN_PUBLISHED_RELEASE.contains("gh release download"));

    let publish = SIGN_PUBLISHED_RELEASE
        .rfind("gh release edit \"$TAG\" --repo \"$REPO\" --draft=false")
        .expect("the operator gate must publish the checked draft");
    let digest_check = SIGN_PUBLISHED_RELEASE
        .rfind("[ \"$declared\" = \"$observed\" ]")
        .expect("the operator gate must check every manifest asset digest");
    assert!(
        digest_check < publish,
        "all draft asset digests must be checked before irreversible publication"
    );
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
fn citation_metadata_names_the_workspace_release() {
    let citation = parse_yaml(CITATION);
    assert_eq!(
        citation["version"].as_str(),
        Some(env!("CARGO_PKG_VERSION")),
        "CITATION.cff and the workspace package identity must move together"
    );

    if let Some(released) = citation.get("date-released") {
        let released = released
            .as_str()
            .expect("date-released must use an ISO date string");
        assert!(
            released.len() == 10
                && released.as_bytes()[4] == b'-'
                && released.as_bytes()[7] == b'-'
                && released
                    .bytes()
                    .enumerate()
                    .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit()),
            "date-released must use YYYY-MM-DD"
        );
    }
}

#[test]
fn installers_verify_release_bytes_and_do_not_ship_a_signer() {
    assert!(!INSTALLER.contains("vela-signer"));
    assert!(!INSTALLER.contains("science.vela.signer.policy"));
    assert!(INSTALLER.contains("VELA_EXPECTED_SHA256"));
}

/// `docs/CONTINUITY.md` requires installing and verifying without the provider.
///
/// The installer verified through `gh attestation verify --signer-workflow`,
/// which is GitHub's OIDC provenance service, and refused to run at all without
/// `gh`. So the documented obligation was unmeetable by the one script every
/// new user runs. It now prefers the signed release manifest, which needs only
/// OpenSSH and a checksum and works against a mirror or a local directory.
#[test]
fn the_installer_can_verify_without_the_provider() {
    assert!(
        INSTALLER.contains("ssh-keygen -Y verify"),
        "the installer must be able to verify a release without GitHub's attestation service"
    );
    assert!(
        INSTALLER.contains("VELA_RELEASE_BASE_URL"),
        "the installer must be able to fetch a release from somewhere other than GitHub"
    );
    assert!(
        INSTALLER.contains("VELA_ALLOWED_SIGNERS"),
        "the trust root must be pinnable out of band, not only taken from the serving host"
    );

    // `gh` may be a fallback for releases published before the manifest
    // existed; it may not be a precondition for running the script at all.
    assert!(
        !INSTALLER.contains("GitHub CLI is required"),
        "`gh` must not be a hard requirement of installation"
    );

    // An unsigned manifest must never be reported as having verified anything.
    //
    // The first draft made a manifest-without-signature fatal, which reads well
    // and would have broken every install on the next tag: `release.yml`
    // requires the manifest before publishing and deliberately declines to sign
    // it in CI, so the pipeline produces exactly that state. Falling back is not
    // a downgrade to nothing — `gh attestation verify` is a real check — but the
    // unsigned document must be ignored rather than counted, and anyone who
    // wants the strong path must be able to demand it.
    assert!(
        INSTALLER.contains("VELA_REQUIRE_SIGNED_MANIFEST"),
        "there must be a way to require the provider-independent path"
    );
    assert!(
        INSTALLER.contains("proves nothing on its own and was ignored"),
        "an unsigned manifest must be ignored out loud, not silently trusted"
    );

    // The digest the installer reads is written by `scripts/release_manifest.py`
    // as `sha256:<hex>`. Requiring bare hex matched nothing, so every real
    // install was refused by the check meant to protect it.
    // `conformance/test_release_install.py` runs the two against each other;
    // this only holds the format open so the pair cannot silently re-diverge.
    assert!(
        INSTALLER.contains("(sha256:)?([0-9a-f]{64})"),
        "the installer must accept the digest form release_manifest.py emits"
    );
}

/// The distribution surface speaks the current vocabulary too.
///
/// `wording_contract.rs` walks the binary's whole help tree and both sides of
/// its error surface, and `ecosystem-status.py` scans `crates/`, `schemas/` and
/// `packages/` for the retired identifier spellings. Between them sits the one
/// script every consumer runs before the binary exists, which neither reads: it
/// told anyone who uninstalled Vela that "Frontier data was preserved" for as
/// long as there had been no Frontier to hold data.
///
/// This used to cover `action.yml` for the same reason — the other file a
/// consumer met first. There is no longer an action to meet.
#[test]
fn the_distribution_surface_does_not_name_a_frontier() {
    assert!(
        !INSTALLER.to_ascii_lowercase().contains("frontier"),
        "install.sh still says Frontier where ADR 0039 means Repository"
    );
}
