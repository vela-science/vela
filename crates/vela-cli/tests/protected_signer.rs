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
