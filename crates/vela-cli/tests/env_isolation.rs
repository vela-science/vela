//! A cloned frontier must not be able to configure the operator.
//!
//! Regression tests for the working-tree `.env` injection: `dotenvy`
//! used to ancestor-walk from cwd, so a frontier repo could commit a
//! `.env` that silently set VELA_ACTOR_ID / VELA_HUB_URL /
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
fn run_scrubbed(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new(vela_bin());
    cmd.current_dir(dir).args(args);
    for (k, _) in std::env::vars() {
        if k.starts_with("VELA_") {
            cmd.env_remove(k);
        }
    }
    // Point HOME at the sandbox so the real ~/.vela profile can't leak in.
    cmd.env("HOME", dir);
    cmd.output().expect("spawn vela")
}

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
        run_in(tmp.path(), &["id", "create", "--handle", "t"])
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
    let second = run_in(tmp.path(), &["land", "r.json", "--as", "agent:t", "--json"]);
    assert!(
        second.status.success(),
        "an exact operation retry must reuse its durable result: {second:?}"
    );
    let second_json: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(second_json["route"], "exact_retry");

    // Exactly one item in the sign queue — no twin.
    let q = run_in(tmp.path(), &["sign", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&q.stdout).unwrap();
    assert_eq!(
        v["signable_total"], 1,
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
        run_in(tmp.path(), &["id", "create", "--handle", "t"])
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

/// Pinning a build-tree binary (the `vela` -> `scripts/vela` wrapper trap)
/// warns: its hash churns on every `cargo build`, so the next ceremony would
/// mismatch. The test binary IS `target/debug/vela`, a dev build, so the guard
/// must fire. (It still records the pin — the human asked — it just says so.)
#[test]
fn pin_binary_warns_on_a_dev_build() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    assert!(
        run_in(tmp.path(), &["id", "create", "--handle", "probe"])
            .status
            .success()
    );
    let out = run_in(tmp.path(), &["id", "pin-binary", "--yes"]);
    assert!(out.status.success(), "pin should record: {out:?}");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        text.contains("build-tree binary"),
        "pinning a target/ binary must warn: {text}"
    );
}

/// The poisoned .env sets VELA_ACTOR_ID=agent:evil. If the CLI loaded
/// it, the sign ceremony would refuse with the CUSTODY exit (4). It must
/// instead fail on identity setup / lookup — anything but 4.
#[test]
fn frontier_env_cannot_set_actor_id() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    std::fs::write(
        tmp.path().join(".env"),
        "VELA_ACTOR_ID=agent:evil\nVELA_KEY_PATH=/tmp/evil.key\n",
    )
    .unwrap();

    let out = run_scrubbed(tmp.path(), &["sign", "vpr_x", "--yes", "--reason", "x"]);
    let code = out.status.code().unwrap_or(-1);
    assert_ne!(
        code,
        4,
        "exit 4 means the custody gate saw agent:evil — the .env was loaded: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The poisoned .env sets VELA_ADVICE=0. If loaded, error hints vanish.
/// The hint must still render.
#[test]
fn frontier_env_cannot_mute_advice() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    std::fs::write(tmp.path().join(".env"), "VELA_ADVICE=0\n").unwrap();

    // A partial scripted-confirmation token prints a usage error WITH the
    // corrective hint and fires before any identity lookup. A bare proposal
    // id is now a valid key-free preview, so it is not a usage error.
    let out = run_scrubbed(
        tmp.path(),
        &[
            "sign",
            "vpr_x",
            "--confirm-root",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("hint:"),
        "the hint vanished — VELA_ADVICE=0 leaked from the frontier .env: {stderr}"
    );
}

/// Real environment variables must still work (the cut removed the
/// working-tree file, not env-var support): VELA_ACTOR_ID from the
/// actual process environment reaches the custody gate.
#[test]
fn real_env_still_resolves() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    let mut cmd = Command::new(vela_bin());
    cmd.current_dir(tmp.path())
        .env("HOME", tmp.path())
        .env("VELA_ACTOR_ID", "agent:probe")
        .args(["sign", "vpr_x", "--yes", "--reason", "x"]);
    let out = cmd.output().expect("spawn");
    assert_eq!(
        out.status.code(),
        Some(4),
        "a REAL env VELA_ACTOR_ID=agent: must hit the custody gate: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A stale pin refuses the CEREMONY, not the read-only list: `sign
/// --json` is how agents and the plugin render the queue, and a changed
/// binary must never take that down (only the pen stops).
#[test]
fn stale_pin_blocks_ceremony_not_list() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    let run = |args: &[&str]| {
        Command::new(vela_bin())
            .current_dir(tmp.path())
            .env("HOME", tmp.path())
            .env("VELA_NO_PUBLISH", "1")
            .args(args)
            .output()
            .expect("spawn")
    };
    let out = run(&["id", "create", "--handle", "probe"]);
    assert!(out.status.success(), "{out:?}");
    let out = run(&["id", "pin-binary", "--yes"]);
    assert!(out.status.success(), "pin failed: {out:?}");
    // Rewrite the pin to a hash the binary cannot match ("it changed").
    let pin_path = tmp.path().join(".vela").join("binary-pin.json");
    let mut pin: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pin_path).unwrap()).unwrap();
    pin["sha256"] = serde_json::Value::String("0".repeat(64));
    std::fs::write(&pin_path, serde_json::to_string_pretty(&pin).unwrap()).unwrap();

    // The read-only list still serves.
    let out = run(&["sign", "--json"]);
    assert!(
        out.status.success(),
        "sign --json must stay a plain read under a stale pin: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("signable_total"),
        "list shape missing: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    // The ceremony refuses with the custody exit and names the mismatch.
    let out = run(&["sign", "--frontier", "."]);
    assert_eq!(
        out.status.code(),
        Some(4),
        "a stale pin must stop the ceremony: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("does not match your pin"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Detached exact-byte signing is the same human-key ceremony. It must
    // refuse before reading a key or writing a signature when the binary pin
    // is stale.
    let subject = tmp.path().join("exact-bytes.txt");
    std::fs::write(&subject, b"exact bytes\n").unwrap();
    let out = run(&[
        "sign",
        "exact-bytes.txt",
        "--key",
        "missing-human-key",
        "--json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(4),
        "a stale pin must stop detached signing before key resolution: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let detached_output = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        detached_output.contains("does not match your pin"),
        "{detached_output}"
    );
    assert!(
        !tmp.path().join("exact-bytes.txt.sig.json").exists(),
        "a refused detached ceremony must not write a signature"
    );
}

/// The binary pin holds: pin a copy of the binary, mutate it, and the
/// ceremony refuses with the custody exit. The clear-signing invariant
/// as a regression test.
#[test]
fn tampered_binary_refuses_ceremony() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    let bin_copy = tmp.path().join("vela-bin");
    std::fs::copy(vela_bin(), &bin_copy).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin_copy, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let run = |args: &[&str]| {
        Command::new(&bin_copy)
            .current_dir(tmp.path())
            .env("HOME", tmp.path())
            .env("VELA_NO_PUBLISH", "1")
            .args(args)
            .output()
            .expect("spawn copy")
    };
    // Identity + pin (human act, --yes for the test).
    let out = run(&["id", "create", "--handle", "probe"]);
    assert!(out.status.success(), "{out:?}");
    let out = run(&["id", "pin-binary", "--yes"]);
    assert!(out.status.success(), "pin failed: {out:?}");
    // Tamper.
    let mut bytes = std::fs::read(&bin_copy).unwrap();
    bytes.push(0);
    std::fs::write(&bin_copy, bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin_copy, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = run(&["sign", "--frontier", "."]);
    // Two refusal layers both count: vela's pin check (exit 4), or —
    // on macOS — the kernel killing the copy outright because the
    // mutation broke its code signature (status None = died by
    // signal). Either way, the tampered binary produced no ceremony.
    match out.status.code() {
        Some(4) => {
            let err = String::from_utf8_lossy(&out.stderr);
            assert!(err.contains("does not match your pin"), "{err}");
        }
        None => {} // killed by the OS before main — defense in depth
        other => panic!(
            "a tampered binary must not run the ceremony (got {other:?}): {}",
            String::from_utf8_lossy(&out.stderr)
        ),
    }
}
