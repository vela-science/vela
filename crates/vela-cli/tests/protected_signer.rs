use std::process::Command;

use tempfile::TempDir;

fn run(home: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(args)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_NO_USER_INTERACTION", "1")
        .output()
        .expect("run isolated Vela")
}

fn write_protected_identity(home: &std::path::Path) {
    let vela_home = home.join(".vela");
    std::fs::create_dir_all(&vela_home).unwrap();
    let public_key = "4".repeat(64);
    let identity = serde_json::json!({
        "version": "2.0",
        "actor_id": "reviewer:fixture",
        "actor_type": "human",
        "key_path": "",
        "pubkey": public_key,
        "signer": {
            "kind": "helper",
            "provider": "os_store",
            "key_id": format!("reviewer:fixture:{public_key}"),
            "public_key": public_key,
            "protection_grade": "user_session",
            "mode": "session",
            "helper_sha256": format!("sha256:{}", "a".repeat(64))
        }
    });
    std::fs::write(
        vela_home.join("identity.json"),
        format!("{}\n", serde_json::to_string_pretty(&identity).unwrap()),
    )
    .unwrap();
}

#[test]
fn protected_identity_cannot_move_binary_pin_through_legacy_yes_flag() {
    let home = TempDir::new().unwrap();
    write_protected_identity(home.path());

    let output = run(home.path(), &["id", "pin-binary", "--yes"]);
    assert!(!output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(combined.contains("cannot move its Vela binary pin"));
    assert!(!home.path().join(".vela/binary-pin.json").exists());
}

#[test]
fn automated_identity_creation_fails_before_opening_platform_ui() {
    let home = TempDir::new().unwrap();

    let output = run(
        home.path(),
        &["id", "create", "--handle", "no-prompt-fixture", "--json"],
    );
    assert!(!output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains(
            "user interaction is disabled; refusing to authenticate protected enrollment"
        )
    );
    assert!(!home.path().join(".vela/identity.json").exists());
    assert!(!home.path().join(".vela/signer-session.json").exists());
}

#[test]
fn simple_protect_command_uses_safe_defaults() {
    let home = TempDir::new().unwrap();
    let output = run(home.path(), &["id", "protect", "--json"]);
    assert!(!output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(combined.contains("no identity configured"));
    assert!(!combined.contains("requires both --user-presence"));
}

#[test]
fn lock_removes_only_local_session_state_and_hides_provider_jargon() {
    let home = TempDir::new().unwrap();
    write_protected_identity(home.path());
    let session = home.path().join(".vela/signer-session.json");
    let identity = home.path().join(".vela/identity.json");
    let identity_before = std::fs::read(&identity).unwrap();
    std::fs::write(&session, b"invalid fixture session").unwrap();

    let show = run(home.path(), &["id", "show"]);
    assert!(show.status.success());
    let human = String::from_utf8_lossy(&show.stdout);
    assert!(human.contains("approval: protected · session"));
    assert!(human.contains("session:  invalid"));
    assert!(!human.contains("os_store"));

    let lock = run(home.path(), &["id", "lock", "--json"]);
    assert!(lock.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&lock.stdout).unwrap();
    assert_eq!(payload["command"], "id.lock");
    assert_eq!(payload["session"], "closed");
    assert_eq!(payload["changed"], true);
    assert_eq!(payload["identity_changed"], false);
    assert_eq!(payload["frontier_changed"], false);
    assert!(!session.exists());
    assert_eq!(std::fs::read(&identity).unwrap(), identity_before);

    let repeat = run(home.path(), &["id", "lock", "--json"]);
    assert!(repeat.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&repeat.stdout).unwrap();
    assert_eq!(payload["changed"], false);
}
