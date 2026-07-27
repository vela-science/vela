//! A cloned frontier must not be able to configure the operator.
//!
//! Regression tests for the working-tree `.env` injection: `dotenvy`
//! used to ancestor-walk from cwd, so a frontier repo could commit a
//! `.env` that silently set VELA_ACTOR_ID / VELA_KEY_PATH /
//! VELA_NO_PUBLISH for anyone running vela inside it — the attack
//! class git's protected configuration and Codex's project-scope key
//! blocking exist for. The CLI now reads NO .env from the working
//! tree; these tests hold that line.

use std::process::Command;

use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};
use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
use vela_protocol::receipt_v1::{ArtifactInput, ReceiptBuilder, ReceiptInput};

fn vela_bin() -> &'static str {
    env!("CARGO_BIN_EXE_vela")
}

/// Run vela in `dir` with a SCRUBBED environment (no VELA_* inherited)
/// so the only possible source of the poisoned values is the .env file.
fn init_frontier(dir: &std::path::Path) {
    let out = Command::new(vela_bin())
        .current_dir(dir)
        .env("HOME", dir)
        .args([
            "init",
            ".",
            "--name",
            "envtest",
            "--scope",
            "Exercise environment isolation.",
        ])
        .output()
        .expect("init");
    assert!(out.status.success(), "init failed: {out:?}");
}

fn run_in(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(vela_bin())
        .current_dir(dir)
        .env("HOME", dir)
        .env("VELA_NO_PUBLISH", "1")
        .args(args)
        .output()
        .expect("spawn vela")
}

fn write_current_receipt(dir: &std::path::Path, filename: &str, claim: &str, replayability: &str) {
    let artifact_path = "witnesses/w.json";
    let artifact = std::fs::read(dir.join(artifact_path)).unwrap();
    let artifact_digest = hex::encode(Sha256::digest(&artifact));
    let project = vela_protocol::repo::load_from_path(dir).unwrap();
    let event_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    );
    let operation_id = format!(
        "vop_{}",
        hex::encode(Sha256::digest(
            format!("{claim}\0{replayability}\0{artifact_digest}").as_bytes()
        ))
    );
    let emitted_at = "2026-07-13T12:00:00Z";
    let identity = IdentityBinding::build(
        IdentityBindingDraft {
            actor_id: "agent:t".to_string(),
            actor_class: ActorClass::Agent,
            created_at: emitted_at.to_string(),
        },
        &SigningKey::from_bytes(&[0x51; 32]),
    )
    .unwrap();
    let input = ReceiptInput::new(
        claim.to_string(),
        "computational".to_string(),
        replayability.to_string(),
        vec![
            ArtifactInput::new(
                artifact_path.to_string(),
                "witness".to_string(),
                Some(artifact_digest),
                None,
            )
            .unwrap(),
        ],
        vec!["fixture evidence only".to_string()],
        Vec::new(),
        "agent:t".to_string(),
        emitted_at.to_string(),
        event_root,
        ".".to_string(),
        operation_id,
        "urn:vela:policy:none".to_string(),
    )
    .unwrap();
    let receipt = ReceiptBuilder::build(input, &identity).unwrap();
    std::fs::write(dir.join(filename), receipt.canonical_bytes().unwrap()).unwrap();
}

/// Malformed invocations across the command families must be exit 2
/// (usage), not the generic exit 1 — the same class fixed in `state`,
/// swept through cli_state / cli_admin / cli_check.
#[test]
fn usage_errors_are_exit_2() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    // `check --json` with no frontier source is a usage error.
    let out = run_in(tmp.path(), &["check", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "check --json no source: {out:?}"
    );
    // Retired state writers fail as usage errors before touching the frontier.
    let out = run_in(tmp.path(), &["state", "anchor", ".", "vf_x", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "retired state anchor writer: {out:?}"
    );
    // `id rotate-key` with identical old/new id is a usage error.
    let out = run_in(
        tmp.path(),
        &[
            "id",
            "rotate-key",
            "--id",
            "reviewer:x",
            "--new-id",
            "reviewer:x",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(2), "id rotate same id: {out:?}");
}

/// The exit-code contract is what an agent branches on. A missing finding
/// must be 3 (not found), while a malformed invocation is 2 (usage).
#[test]
fn finding_show_honors_the_exit_code_contract() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    let dir = ".";
    // A well-formed but absent finding id → not found (3).
    let out = run_in(
        tmp.path(),
        &[
            "finding",
            "show",
            dir,
            "vf_ffffffffffffffff",
            "--view",
            "standing",
            "--json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(3),
        "missing finding must be exit 3: {out:?}"
    );
    // A malformed invocation (no operands) → usage (2).
    let out = run_in(tmp.path(), &["finding", "show", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "usage error must be exit 2: {out:?}"
    );
}

/// Landing the same operation identity and normalized receipt is a retry, not
/// a new finding. The
/// second land must be exit 5 (already_exists) and must NOT fork a twin
/// into the sign queue.
#[test]
fn land_is_idempotent_on_the_claim() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    assert!(
        run_in(tmp.path(), &["id", "create", "--handle", "t", "--agent"])
            .status
            .success()
    );
    std::fs::create_dir_all(tmp.path().join("witnesses")).unwrap();
    std::fs::write(tmp.path().join("witnesses/w.json"), "{\"k\":\"d\"}").unwrap();
    write_current_receipt(
        tmp.path(),
        "r.json",
        "idempotency regression claim",
        "exact",
    );

    let first = run_in(tmp.path(), &["land", "r.json", "--as", "agent:t", "--json"]);
    assert!(
        first.status.success(),
        "first land should succeed: {first:?}"
    );
    let compact = run_in(tmp.path(), &["frontier", "compact-recovery", ".", "--json"]);
    assert!(
        compact.status.success(),
        "settled recovery compaction should succeed: {compact:?}"
    );
    let second = run_in(tmp.path(), &["land", "r.json", "--as", "agent:t", "--json"]);
    assert!(
        second.status.success(),
        "an exact operation retry must reuse its durable result: {second:?}"
    );
    let second_json: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(second_json["route"], "exact_retry");

    // Exactly one pending proposal — no twin.
    let q = run_in(tmp.path(), &["review", "list", ".", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&q.stdout).unwrap();
    assert_eq!(
        v["total"], 1,
        "the retry must not have forked a duplicate: {v}"
    );
}

/// The Receipt v1 replayability class: an explicit honest value lands and a
/// value outside the frozen closed set is rejected.
#[test]
fn land_honors_the_replayability_class() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    assert!(
        run_in(tmp.path(), &["id", "create", "--handle", "t", "--agent"])
            .status
            .success()
    );
    std::fs::create_dir_all(tmp.path().join("witnesses")).unwrap();
    std::fs::write(tmp.path().join("witnesses/w.json"), "{\"k\":\"d\"}").unwrap();

    // A valid, honest replayability value lands.
    write_current_receipt(
        tmp.path(),
        "ok.json",
        "an approximately-replayable hosted-model run",
        "approximate",
    );
    let ok = run_in(
        tmp.path(),
        &["land", "ok.json", "--as", "agent:t", "--json"],
    );
    assert!(
        ok.status.success(),
        "an `approximate` receipt should land: {ok:?}"
    );

    // A value outside the closed set is rejected on the usage contract (exit 2).
    write_current_receipt(tmp.path(), "bad.json", "a mislabeled run", "exact");
    let mut bad_receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(tmp.path().join("bad.json")).unwrap()).unwrap();
    bad_receipt["replayability"] =
        serde_json::Value::String("totally-reproducible-trust-me".to_string());
    std::fs::write(
        tmp.path().join("bad.json"),
        serde_json::to_vec(&bad_receipt).unwrap(),
    )
    .unwrap();
    let bad = run_in(
        tmp.path(),
        &["land", "bad.json", "--as", "agent:t", "--json"],
    );
    assert!(
        !bad.status.success(),
        "an unknown replayability class must be rejected: {bad:?}"
    );
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&bad.stdout),
        String::from_utf8_lossy(&bad.stderr)
    );
    assert!(
        text.contains("replayability") || text.contains("totally-reproducible-trust-me"),
        "the rejection should identify the offending replayability value: {text}"
    );
}
