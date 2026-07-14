//! Integration tests pinning the consolidated CLI surface. After the
//! dev-only cleanup, each concept has exactly ONE spelling: the
//! acting-identity flag is `--as` (no `--reviewer`/`--actor`/`--by`), the
//! key flag is `--key` (no `--private-key`), and the finding-mutation verbs
//! live only under `vela finding <verb>` (no top-level `vela note`). These
//! run the built `vela` binary so they catch surface drift the clap-tree
//! unit tests can't.

use std::process::{Command, Output};
use tempfile::TempDir;
use vela_protocol::access_tier::AccessTier;
use vela_protocol::bundle::{
    Artifact, ArtifactAvailability, ArtifactDisclosure, Extraction, LocatorIntegrity, Provenance,
};

fn vela(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(args)
        .output()
        .expect("run vela")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn combined(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// The acting-identity flag is `--as` and ONLY `--as` — the retired
/// `--reviewer`/`--actor`/`--by` spellings must be rejected (one name).
#[test]
fn identity_flag_is_canonical_as_only() {
    let ok = vela(&[
        "land",
        "/tmp/vela_nonexistent_receipt.json",
        "--as",
        "reviewer:w",
    ]);
    assert!(
        !stderr(&ok).contains("unexpected argument"),
        "`land --as` should parse, got: {}",
        stderr(&ok)
    );
    for retired in ["--reviewer", "--actor", "--by"] {
        let out = vela(&[
            "land",
            "/tmp/vela_nonexistent_receipt.json",
            retired,
            "reviewer:w",
        ]);
        assert!(
            stderr(&out).contains("unexpected argument") || stderr(&out).contains(retired),
            "retired alias `{retired}` should be rejected, got: {}",
            stderr(&out)
        );
    }
}

/// `sign` takes `--key` and only `--key`; the retired `attest` top-level
/// 404s outright.
#[test]
fn key_flag_is_canonical_key_only() {
    let ok = vela(&[
        "sign",
        "vpr_nonexistent",
        "--yes",
        "--key",
        "/tmp/nope",
        "--frontier",
        "/tmp/vela_nonexistent_frontier",
    ]);
    assert!(
        !stderr(&ok).contains("unexpected argument"),
        "`sign --key` should parse, got: {}",
        stderr(&ok)
    );
    let retired = vela(&["attest", "apply", "/tmp/x.json", "--key", "/tmp/nope"]);
    assert!(
        combined(&retired).contains("unknown or non-release command"),
        "retired `attest` top-level should 404, got: {}",
        combined(&retired)
    );
}

/// Every retired top-level spelling 404s with the release-surface error —
/// no aliases, no shims, the porcelain is the porcelain.
#[test]
fn retired_top_level_verbs_404() {
    for verb in [
        "verify",
        "history",
        "accept-batch",
        "normalize",
        "ingest",
        "claim",
        "campaign",
        "lean",
        "attempt",
        "transfer",
        "experiment",
        "registry",
        "attest",
        "receipt",
        "publish",
        "clone",
        "workspace",
        // the v0.738 hard cut: ten verbs retired into the loop
        "inbox",
        "propose",
        "accept",
        "review",
        "record",
        "pack",
        "attach",
        "queue",
    ] {
        let out = vela(&[verb, "--help"]);
        assert!(
            combined(&out).contains("unknown or non-release command"),
            "retired verb `{verb}` should 404, got: {}",
            combined(&out)
        );
        assert_eq!(
            out.status.code(),
            Some(2),
            "retired verb `{verb}` should exit 2"
        );
    }
}

/// The folded spellings dispatch: hub, foundry planes, id keygen, state.
#[test]
fn folded_spellings_dispatch() {
    for args in [
        vec!["hub", "--help"],
        vec!["foundry", "campaign", "--help"],
        vec!["foundry", "lean", "--help"],
        vec!["foundry", "attempt", "--help"],
        vec!["foundry", "transfer", "--help"],
        vec!["foundry", "experiment", "--help"],
        vec!["id", "keygen", "--help"],
        vec!["sign", "--help"],
    ] {
        let out = vela(&args);
        assert!(
            !combined(&out).contains("unknown or non-release command"),
            "`{}` should dispatch, got: {}",
            args.join(" "),
            combined(&out)
        );
    }
    // the pre-clap intercepts: `state` (claim-state projection) and `atlas`
    // (cross-frontier math atlas) reach their parsers ahead of clap. A usage
    // error is fine; a 404 ("unknown or non-release command") means the
    // intercept was dropped in a refactor.
    for intercept in ["state", "atlas"] {
        let out = vela(&[intercept]);
        assert!(
            !combined(&out).contains("unknown or non-release command"),
            "`{intercept}` should reach the intercept, got: {}",
            combined(&out)
        );
    }
}

/// Sanity: a genuinely-unknown flag is rejected (so the checks above mean
/// something — the parser doesn't swallow everything).
#[test]
fn unknown_flag_is_rejected() {
    let out = vela(&[
        "land",
        "/tmp/vela_nonexistent_receipt.json",
        "--definitely-not-a-flag",
        "y",
    ]);
    let e = stderr(&out);
    assert!(
        e.contains("unexpected argument") || e.contains("--definitely-not-a-flag"),
        "unknown flag should be rejected, got: {e}"
    );
}

/// The store-level decision verbs still dispatch: `proposals accept` /
/// `proposals reject` survive the hard cut (the porcelain `vela sign` is
/// their ceremony driver). Neither regresses to "unknown or non-release
/// command".
#[test]
fn accept_paths_dispatch() {
    for args in [
        vec!["proposals", "accept", "--help"],
        vec!["proposals", "reject", "--help"],
    ] {
        let out = vela(&args);
        assert!(
            !combined(&out).contains("unknown or non-release command"),
            "`{}` should dispatch, not 404",
            args.join(" ")
        );
    }
}

/// The managed-identity verbs must be reachable through the clap-derived
/// allowlist (the drift that bit `id`). `publish` is retired (ADR 0001
/// Phase 2: git push is publication), so it is no longer asserted here.
#[test]
fn ergonomics_verbs_are_reachable() {
    let out = vela(&["id", "--help"]);
    assert!(
        !combined(&out).contains("unknown or non-release command"),
        "`id` should be reachable"
    );
}

/// The finding-mutation/graph verbs live ONLY under `vela finding <verb>`;
/// the retired top-level spellings (`vela note` …) must now 404.
#[test]
fn finding_verbs_are_nested_only() {
    for verb in ["note", "caveat", "revise", "reject", "retract", "link"] {
        let nested = vela(&["finding", verb, "--help"]);
        assert!(
            !combined(&nested).contains("unknown or non-release command")
                && !combined(&nested).contains("unrecognized subcommand"),
            "`vela finding {verb}` should dispatch"
        );
        let top = vela(&[verb, "--help"]);
        assert!(
            combined(&top).contains("unknown or non-release command"),
            "retired top-level `vela {verb}` should 404, got: {}",
            combined(&top)
        );
    }
    let finding_help = combined(&vela(&["finding", "--help"]));
    assert!(finding_help.contains("note") && finding_help.contains("retract"));
}

fn artifact_frontier() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp frontier");
    let path = tmp.path().join("frontier.json");
    let mut frontier = vela_protocol::project::assemble("artifact-test", vec![], 0, 0, "test");
    frontier.artifacts.push(Artifact {
        id: "va_1111111111111111".to_string(),
        kind: "code".to_string(),
        name: "legacy pointer".to_string(),
        content_hash: format!("sha256:{}", "a".repeat(64)),
        size_bytes: None,
        media_type: None,
        storage_mode: "remote".to_string(),
        disclosure: ArtifactDisclosure::Unknown,
        locator_integrity: LocatorIntegrity::Unknown,
        availability: ArtifactAvailability::Unknown,
        locator: Some("https://example.test/proof.lean".to_string()),
        source_url: Some("https://example.test/proof.lean".to_string()),
        license: Some("MIT".to_string()),
        target_findings: Vec::new(),
        source_id: None,
        provenance: Provenance {
            source_type: "data_release".to_string(),
            doi: None,
            url: Some("https://example.test/proof.lean".to_string()),
            title: "legacy pointer".to_string(),
            authors: Vec::new(),
            year: Some(2026),
            license: Some("MIT".to_string()),
            publisher: None,
            funders: Vec::new(),
            extraction: Extraction::default(),
            review: None,
            contributions: Vec::new(),
        },
        metadata: Default::default(),
        review_state: None,
        retracted: false,
        access_tier: AccessTier::Public,
        created: "2026-07-12T00:00:00Z".to_string(),
    });
    vela_protocol::repo::save_to_path(&path, &frontier).expect("save frontier");
    (tmp, path)
}

#[test]
fn artifact_retract_is_draft_only_json_porcelain() {
    let (_tmp, path) = artifact_frontier();
    let out = vela(&[
        "artifact",
        "retract",
        path.to_str().unwrap(),
        "va_1111111111111111",
        "--reason",
        "legacy source is not immutable",
        "--as",
        "agent:cleanup",
        "--json",
    ]);
    assert!(out.status.success(), "{}", combined(&out));
    assert!(
        stderr(&out).is_empty(),
        "JSON command leaked stderr: {}",
        stderr(&out)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("one JSON success object");
    let keys = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        keys,
        [
            "artifact_id",
            "command",
            "ok",
            "proposal_id",
            "route",
            "status"
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    );
    assert_eq!(value["command"], "artifact retract");
    assert_eq!(value["status"], "pending_review");
    assert_eq!(value["route"], "deferred");

    let loaded = vela_protocol::repo::load_from_path(&path).expect("reload frontier");
    assert!(!loaded.artifacts[0].retracted);
    assert_eq!(loaded.events.len(), 1);
    assert_eq!(loaded.proposals.len(), 1);
}

#[test]
fn artifact_retract_has_typed_missing_and_no_apply_escape_hatch() {
    let (_tmp, path) = artifact_frontier();
    let missing = vela(&[
        "artifact",
        "retract",
        path.to_str().unwrap(),
        "va_2222222222222222",
        "--reason",
        "not present",
        "--as",
        "agent:cleanup",
        "--json",
    ]);
    assert_eq!(missing.status.code(), Some(3));
    assert!(stderr(&missing).is_empty());
    let error: serde_json::Value =
        serde_json::from_slice(&missing.stdout).expect("one JSON error object");
    assert_eq!(error["ok"], false);

    let apply = vela(&[
        "artifact",
        "retract",
        path.to_str().unwrap(),
        "va_1111111111111111",
        "--reason",
        "legacy",
        "--as",
        "agent:cleanup",
        "--apply",
    ]);
    assert_eq!(apply.status.code(), Some(2));
    assert!(combined(&apply).contains("--apply"));

    let top = vela(&["retract", "--help"]);
    assert!(combined(&top).contains("unknown or non-release command"));
}

/// `reproduce-external` is a normal parsed command. Missing positionals must
/// therefore use Clap's bounded usage failure, and NO_COLOR must remain clean.
#[test]
fn parsed_external_reproduction_has_bounded_colorless_usage_errors() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_vela"))
        .arg("reproduce-external")
        .env("NO_COLOR", "1")
        .output()
        .expect("run vela");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("the following required arguments were not provided")
            && err.contains("Usage: vela reproduce-external"),
        "parsed usage error is incomplete: {err}"
    );
    assert!(
        !err.contains('\u{1b}'),
        "under NO_COLOR the intercept must emit no ANSI escape, got: {err:?}"
    );
}

/// Interactive prompts must refuse cleanly on non-terminal stdin (piped /
/// CI) rather than hang on empty reads or silently assume "no" (gh/clig.dev).
/// `id pin-binary` is the simplest guarded prompt (no binary-pin pre-gate).
#[test]
fn prompts_refuse_piped_stdin() {
    use std::process::Stdio;
    let home = tempfile::TempDir::new().expect("temporary Vela home");
    let setup = Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(["id", "create", "--handle", "prompt-test"])
        .env("HOME", home.path())
        .env("VELA_NO_PUBLISH", "1")
        .output()
        .expect("create fixture identity");
    assert!(
        setup.status.success(),
        "fixture identity setup failed: {}",
        combined(&setup)
    );
    let out = Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(["id", "pin-binary"])
        .env("HOME", home.path())
        .env("VELA_NO_PUBLISH", "1")
        .stdin(Stdio::null()) // /dev/null is not a tty
        .output()
        .expect("run vela");
    let err = stderr(&out);
    assert!(
        err.contains("not an interactive terminal"),
        "piped `id pin-binary` must refuse to prompt, got: {err}"
    );
    assert_ne!(
        out.status.code(),
        Some(0),
        "a refused prompt must not exit 0"
    );
}

/// The `docs/CLI.md` promise — every porcelain verb takes `--json` — was
/// false for `config unset` and `policy sign/revoke`. Pin the two fixes:
/// `--json` parses, and a signing verb under `--json` is non-interactive
/// (requires `--yes`), never leaking a prompt into the stream.
#[test]
fn json_is_offered_where_the_contract_promises() {
    let unset = vela(&["config", "unset", "some.unknown.key", "--json"]);
    assert!(
        !stderr(&unset).contains("unexpected argument"),
        "`config unset --json` must parse, got: {}",
        stderr(&unset)
    );
    let sign = vela(&["policy", "sign", "examples/sidon-sets", "--json"]);
    assert!(
        combined(&sign).contains("requires --yes"),
        "`policy sign --json` must require --yes (JSON is non-interactive), got: {}",
        combined(&sign)
    );
    assert_eq!(
        sign.status.code(),
        Some(2),
        "the requires-yes refusal is a usage error"
    );
}
