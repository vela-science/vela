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
        .args(["init", ".", "--name", "envtest", "--no-git"])
        .output()
        .expect("init");
    assert!(out.status.success(), "init failed: {out:?}");
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

    // A scripted sign without --yes prints a usage error WITH the
    // corrective hint (and fires before any identity lookup).
    let out = run_scrubbed(tmp.path(), &["sign", "vpr_x"]);
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
