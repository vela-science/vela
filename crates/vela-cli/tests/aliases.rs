//! Integration tests pinning the consolidated CLI surface. After the
//! dev-only cleanup, each concept has exactly ONE spelling: the
//! acting-identity flag is `--as` (no `--reviewer`/`--actor`/`--by`), the
//! key flag is `--key` (no `--private-key`), and `finding` is read-only.
//! Submission v1 plus `submit` is the producer write boundary. These
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
        "submit",
        "/tmp/vela_nonexistent_submission.json",
        "--as",
        "reviewer:w",
    ]);
    assert!(
        !stderr(&ok).contains("unexpected argument"),
        "`submit --as` should parse, got: {}",
        stderr(&ok)
    );
    for retired in ["--reviewer", "--actor", "--by"] {
        let out = vela(&[
            "submit",
            "/tmp/vela_nonexistent_submission.json",
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
        combined(&retired).contains("unknown command"),
        "retired `attest` top-level should 404, got: {}",
        combined(&retired)
    );
}

/// Unknown historical spellings fail with the bounded 0.9 surface error.
#[test]
fn retired_top_level_verbs_404() {
    for verb in [
        "history",
        "accept-batch",
        "normalize",
        "ingest",
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
        "record",
        "pack",
        "attach",
        "queue",
    ] {
        let out = vela(&[verb, "--help"]);
        assert!(
            combined(&out).contains("unknown command"),
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

/// Atlas left the core binary. Every old subcommand stops at the same hint.
#[test]
fn atlas_ingest_writers_are_retired() {
    for subcommand in ["ingest", "ingest-source", "ingest-graph"] {
        let out = vela(&["atlas", subcommand]);
        let text = combined(&out);
        assert_eq!(out.status.code(), Some(2), "{subcommand}: {text}");
        assert!(text.contains("retired from the core binary"), "{text}");
        assert!(text.contains("Canopus verifier profile"), "{text}");
    }
}

/// The compact setup and read/review nouns remain reachable.
#[test]
fn folded_spellings_dispatch() {
    for args in [
        vec!["id", "keygen", "--help"],
        vec!["review", "--help"],
        vec!["claim", "show", "--help"],
        vec!["frontier", "diff", "--help"],
    ] {
        let out = vela(&args);
        assert!(
            !combined(&out).contains("unknown or non-release command"),
            "`{}` should dispatch, got: {}",
            args.join(" "),
            combined(&out)
        );
    }
    for retired in ["hub", "foundry", "state", "atlas"] {
        let out = vela(&[retired]);
        assert!(
            combined(&out).contains("retired"),
            "`{retired}` should return a migration hint, got: {}",
            combined(&out)
        );
    }
}

#[test]
fn review_help_directs_known_proposals_to_the_terminal_read_surface() {
    let out = vela(&["review", "--help"]);
    assert!(out.status.success(), "{}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("KNOWN PROPOSAL"), "{text}");
    assert!(text.contains("start with `vela review show`"), "{text}");
    assert!(text.contains("intentionally absent from"), "{text}");
    assert!(
        text.contains("accepted `claim show` and `log` views"),
        "{text}"
    );
}

/// Repository-authority decisions intentionally use direct action verbs and
/// have no copied confirmation, legacy batch, key-path, or non-interactive
/// approval inputs. Clap must reject those spellings before frontier or
/// identity resolution.
#[test]
fn direct_review_actions_have_no_batch_key_yes_or_wildcard_surface() {
    for action in ["accept", "reject"] {
        let help = vela(&["review", action, "--help"]);
        assert!(help.status.success(), "{}", combined(&help));
        let help = combined(&help);
        assert!(help.contains("--reason"), "missing --reason: {help}");
        for forbidden in [
            "--confirm-root",
            "--confirm-at",
            "--key",
            "--yes",
            "--batch",
            "--all",
        ] {
            assert!(!help.contains(forbidden), "leaked {forbidden}: {help}");
            let out = vela(&[
                "review",
                action,
                "/tmp/vela-nonexistent-frontier",
                "vpr_0000000000000000",
                "--reason",
                "fixture",
                forbidden,
            ]);
            assert_eq!(
                out.status.code(),
                Some(2),
                "{forbidden}: {}",
                combined(&out)
            );
            assert!(
                combined(&out).contains("unexpected argument"),
                "{forbidden}: {}",
                combined(&out)
            );
        }
    }

    let wildcard = vela(&[
        "review",
        "reject",
        "/tmp/vela-nonexistent-frontier",
        "*",
        "--reason",
        "fixture",
    ]);
    assert_ne!(wildcard.status.code(), Some(0));
    assert!(!combined(&wildcard).contains("signed"));
}

#[test]
fn legacy_policy_retirement_writer_is_retired_without_touching_retained_bytes() {
    const POLICY_ID: &str = "vap_e0abc750544408e637bd90e0661bac15";
    let frontier = TempDir::new().unwrap();
    let initialized = vela(&[
        "init",
        frontier.path().to_str().unwrap(),
        "--name",
        "legacy-policy-retirement-cli",
        "--scope",
        "Retire one retained prelaunch policy pair without accepting it.",
        "--json",
    ]);
    assert!(initialized.status.success(), "{}", combined(&initialized));
    let policies = frontier.path().join(".vela/policies");
    std::fs::create_dir_all(&policies).unwrap();
    std::fs::write(
        policies.join("active.json"),
        format!("{{\"schema\":\"vela.acceptance_policy.prelaunch\",\"id\":\"{POLICY_ID}\"}}\n"),
    )
    .unwrap();
    std::fs::write(
        policies.join("active.sig.json"),
        format!("{{\"policy_id\":\"{POLICY_ID}\",\"signature\":\"historical\"}}\n"),
    )
    .unwrap();
    let before = vela_protocol::repo::load_from_path(frontier.path()).unwrap();
    let before_event_count = before.events.len();
    let before_proposal_count = before.proposals.len();

    let out = vela(&[
        "policy",
        "retire-legacy",
        frontier.path().to_str().unwrap(),
        "--reason",
        "retire unsupported prelaunch bytes",
        "--as",
        "agent:test",
        "--json",
    ]);
    assert!(!out.status.success(), "{}", combined(&out));
    assert!(combined(&out).contains("unrecognized subcommand 'retire-legacy'"));
    let project = vela_protocol::repo::load_from_path(frontier.path()).unwrap();
    assert_eq!(project.proposals.len(), before_proposal_count);
    assert_eq!(
        project.events.len(),
        before_event_count,
        "failed preparation must not create a decision event"
    );
    assert!(policies.join("active.json").exists());
    assert!(policies.join("active.sig.json").exists());
}

/// Sanity: a genuinely-unknown flag is rejected (so the checks above mean
/// something — the parser doesn't swallow everything).
#[test]
fn unknown_flag_is_rejected() {
    let out = vela(&[
        "submit",
        "/tmp/vela_nonexistent_submission.json",
        "--definitely-not-a-flag",
        "y",
    ]);
    let e = stderr(&out);
    assert!(
        e.contains("unexpected argument") || e.contains("--definitely-not-a-flag"),
        "unknown flag should be rejected, got: {e}"
    );
}

/// The public decision surface is singular: `vela sign`. Direct proposal
/// decision subcommands must not return after the prelaunch cut.
#[test]
fn direct_proposal_decision_paths_are_absent() {
    for args in [
        vec!["proposals", "accept", "--help"],
        vec!["proposals", "reject", "--help"],
    ] {
        let out = vela(&args);
        assert!(
            !out.status.success(),
            "`{}` unexpectedly exists",
            args.join(" ")
        );
        let text = combined(&out);
        assert!(
            text.contains("retired") && text.contains("vela review"),
            "`{}` should be rejected by clap: {text}",
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

#[test]
fn ordinary_identity_and_policy_help_hide_legacy_key_ceremonies() {
    let identity = combined(&vela(&["id", "--help"]));
    for hidden in ["pin-binary", "import", "keygen"] {
        assert!(
            !identity.contains(hidden),
            "id help leaked {hidden}: {identity}"
        );
    }
    for visible in ["create", "show"] {
        assert!(
            identity.contains(visible),
            "id help omitted {visible}: {identity}"
        );
    }
    for retired in ["protect", "lock", "pin-binary"] {
        assert!(
            !identity.contains(retired),
            "id help leaked retired ceremony {retired}: {identity}"
        );
    }

    let policy = combined(&vela(&["policy", "--help"]));
    for hidden in [
        "  suggest",
        "  draft",
        "  decide",
        "  sign",
        "  revoke",
        "retire-legacy",
    ] {
        assert!(
            !policy.contains(hidden),
            "policy help leaked {hidden}: {policy}"
        );
    }
    for visible in ["show", "test", "evaluate-proposal", "log"] {
        assert!(
            policy.contains(visible),
            "policy help omitted {visible}: {policy}"
        );
    }

    // Local historical identity recovery remains available until every
    // selected frontier completes repository-authority migration. Policy
    // writers are retired rather than hidden aliases.
    assert!(vela(&["id", "keygen", "--help"]).status.success());
    assert!(!vela(&["policy", "sign", "--help"]).status.success());
}

#[test]
fn claim_is_read_only_and_legacy_writer_bypasses_are_absent() {
    let show = vela(&["claim", "show", "--help"]);
    assert!(show.status.success(), "{}", combined(&show));

    for verb in [
        "add",
        "supersede",
        "note",
        "caveat",
        "revise",
        "review",
        "reject",
        "retract",
        "contribution",
    ] {
        let nested = vela(&["claim", verb, "--help"]);
        assert!(
            !nested.status.success()
                && (combined(&nested).contains("unrecognized subcommand")
                    || combined(&nested).contains("unexpected argument")),
            "`vela claim {verb}` unexpectedly exists: {}",
            combined(&nested)
        );
    }
    let claim_help = combined(&vela(&["claim", "--help"]));
    assert!(claim_help.contains("Submission v1") && claim_help.contains("vela submit"));

    let retired = combined(&vela(&["finding", "show", "--help"]));
    assert!(retired.contains("retired") && retired.contains("vela claim show"));

    for args in [
        vec!["proposals", "import", "--help"],
        vec!["sign", "--batch", "/tmp/fidelity.json"],
    ] {
        let out = vela(&args);
        assert!(
            !out.status.success(),
            "`{}` unexpectedly exists",
            args.join(" ")
        );
        let text = combined(&out);
        assert!(
            text.contains("unrecognized subcommand")
                || text.contains("unexpected argument")
                || text.contains("retired")
                || text.contains("unknown command"),
            "`{}` should be rejected by clap: {text}",
            args.join(" ")
        );
    }
}

#[test]
fn direct_writer_bypasses_are_absent() {
    for args in [
        vec!["claim", "link", "add", "--help"],
        vec!["frontier", "add-dep", "--help"],
        vec!["actor", "rotate", "--help"],
    ] {
        let out = vela(&args);
        assert!(
            !out.status.success(),
            "`{}` unexpectedly exists",
            args.join(" ")
        );
        let text = combined(&out);
        assert!(
            text.contains("unrecognized subcommand") || text.contains("unexpected argument"),
            "`{}` should be rejected by clap: {text}",
            args.join(" ")
        );
    }

    for verb in ["anchor", "unanchor"] {
        let out = vela(&["state", verb, "."]);
        let text = combined(&out);
        assert_eq!(out.status.code(), Some(2), "{verb}: {text}");
        assert!(text.contains("state` retired"), "{verb}: {text}");
        assert!(text.contains("vela claim show"), "{verb}: {text}");
    }
}

#[test]
fn retired_actor_writers_are_rejected_by_clap() {
    let arbitrary = vela(&[
        "actor",
        "add",
        "/tmp/never-opened-frontier",
        "reviewer:other",
        "--pubkey",
        &"00".repeat(32),
    ]);
    assert_eq!(arbitrary.status.code(), Some(2));
    assert!(
        combined(&arbitrary).contains("unrecognized subcommand")
            || combined(&arbitrary).contains("unexpected argument")
    );
    for action in ["activate", "bootstrap"] {
        let output = vela(&["actor", action, "."]);
        assert_eq!(output.status.code(), Some(2));
    }
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
    assert!(combined(&top).contains("unknown command"));
}

/// `reproduce-external` stops at the 0.9 Canopus migration hint.
#[test]
fn parsed_external_reproduction_has_bounded_colorless_usage_errors() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_vela"))
        .arg("reproduce-external")
        .env("NO_COLOR", "1")
        .output()
        .expect("run vela");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("retired in 0.900") && err.contains("Canopus verifier profile"),
        "migration hint is incomplete: {err}"
    );
    assert!(
        !err.contains('\u{1b}'),
        "under NO_COLOR the intercept must emit no ANSI escape, got: {err:?}"
    );
}

/// The retired binary-pin ceremony must stop at parsing and never recreate a
/// local identity or binary-pin side channel.
#[test]
fn retired_binary_pin_ceremony_has_no_prompt_or_write_path() {
    let home = tempfile::TempDir::new().expect("temporary Vela home");
    let out = Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(["id", "pin-binary"])
        .env("HOME", home.path())
        .env("VELA_NO_PUBLISH", "1")
        .output()
        .expect("run vela");
    let err = stderr(&out);
    assert!(
        err.contains("unrecognized subcommand"),
        "retired `id pin-binary` must be rejected by clap, got: {err}"
    );
    assert_ne!(out.status.code(), Some(0));
    assert!(!home.path().join(".vela/binary-pin.json").exists());
}

/// JSON porcelain remains available on active commands. Retired policy
/// writers stay unreachable rather than accepting hidden compatibility flags.
#[test]
fn json_is_offered_where_the_contract_promises() {
    let unset = vela(&["config", "unset", "some.unknown.key", "--json"]);
    assert!(
        !stderr(&unset).contains("unexpected argument"),
        "`config unset --json` must parse, got: {}",
        stderr(&unset)
    );
    let sign = vela(&["policy", "sign", "examples/erdos-formalization", "--json"]);
    assert!(
        combined(&sign).contains("unrecognized subcommand 'sign'"),
        "`policy sign` must remain retired, got: {}",
        combined(&sign)
    );
    assert_eq!(sign.status.code(), Some(2));
}
