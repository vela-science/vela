use std::process::Command;

use serde_json::json;
use tempfile::TempDir;

fn write_witness() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("witness.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({
            "kind": "sidon",
            "n": 3,
            "points": [[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]],
            "claimed_size": 4
        }))
        .unwrap(),
    )
    .unwrap();
    (dir, path)
}

#[test]
fn exact_claim_option_accepts_only_a_faithful_bound() {
    let (_dir, path) = write_witness();
    let binary = env!("CARGO_BIN_EXE_vela-verify");
    let accepted = Command::new(binary)
        .args([
            "--claim",
            "There exists a Sidon subset of {0,1}^3 with at least 4 elements.",
        ])
        .arg(&path)
        .output()
        .unwrap();
    assert!(accepted.status.success());
    assert!(
        String::from_utf8(accepted.stdout)
            .unwrap()
            .contains("\"ok\":true")
    );

    let inflated = Command::new(binary)
        .args([
            "--claim",
            "There exists a Sidon subset of {0,1}^3 with at least 5 elements.",
        ])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!inflated.status.success());
    assert!(
        String::from_utf8(inflated.stdout)
            .unwrap()
            .contains("\"ok\":false")
    );
}

#[test]
fn witness_only_compatibility_remains_available() {
    let (_dir, path) = write_witness();
    let output = Command::new(env!("CARGO_BIN_EXE_vela-verify"))
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success());
}
