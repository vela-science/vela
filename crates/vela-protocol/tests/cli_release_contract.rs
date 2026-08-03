use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

fn vela_bin() -> PathBuf {
    if let Ok(env_path) = std::env::var("VELA_BIN") {
        return PathBuf::from(env_path);
    }
    if let Ok(env_path) = std::env::var("CARGO_BIN_EXE_vela") {
        return PathBuf::from(env_path);
    }
    // CI may have built only the release binary; check both locations.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let debug = manifest.join("../../target/debug/vela");
    if debug.is_file() {
        return debug;
    }
    let release = manifest.join("../../target/release/vela");
    if release.is_file() {
        return release;
    }
    debug
}

fn run_text(args: &[&str]) -> String {
    let output = Command::new(vela_bin())
        .args(args)
        .output()
        .expect("failed to run vela");
    assert!(
        output.status.success(),
        "vela command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("command output was not UTF-8")
}

fn run_expect_failure(args: &[&str]) -> String {
    let output = Command::new(vela_bin())
        .args(args)
        .output()
        .expect("failed to run vela");
    assert!(
        !output.status.success(),
        "vela command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn check_missing_frontier_reports_error_without_panic() {
    let tmp = TempDir::new().unwrap();
    let missing = tmp.path().join("missing-frontier.json");

    // `check` rejects anything that is not a current repository origin through
    // one bounded product error. It must not fall through to a legacy loader.
    let error = run_expect_failure(&["check", missing.to_str().unwrap()]);

    assert!(error.contains("verifies only current repository origins"));
    assert!(!error.contains("panicked at"));
}

#[test]
fn advanced_help_uses_current_product_commands() {
    let help = run_text(&["help", "advanced"]);

    for command in [
        "init",
        "status",
        "next",
        "start",
        "submit",
        "show",
        "why",
        "review",
        "check",
        "reproduce",
        "log",
    ] {
        assert!(
            help.contains(&format!("  {command}")),
            "advanced help omitted current product command: {command}"
        );
    }
    assert!(help.contains("check         Replay, signatures, parity, and repository integrity"));
    assert!(help.contains("reproduce     Re-run stored witnesses with frozen verifiers"));
    assert!(help.contains("review        Inspect or perform one exact Proposal lifecycle action"));
    assert!(help.contains("verification  Retain non-authorizing scoped Verification Records"));
    assert!(!help.contains("  id "));

    assert!(!help.contains("bridges derive"));
    assert!(!help.contains("vela workbench"));
    // The help must advertise nothing the binary cannot run.
    for dead in [
        "scout",
        "compile-notes",
        "clinical-trial-import",
        "source-inbox",
        "constellation",
        "federation",
        "  bridge ",
        "  packet ",
        "  bench ",
        "Workbench",
    ] {
        assert!(
            !help.contains(dead),
            "help advanced still advertises removed surface: {dead}"
        );
    }
}

#[test]
fn verification_help_exposes_ordinary_authoring_without_key_flags() {
    let help = run_text(&["verification", "record", "--help"]);

    for flag in [
        "--profile",
        "--method",
        "--property",
        "--complementary",
        "--outcome",
        "--does-not-establish",
        "--independent-of",
        "--shared-dependency",
        "--as",
        "--json",
    ] {
        assert!(
            help.contains(flag),
            "verification record help omitted {flag}"
        );
    }
    assert!(help.contains("<FRONTIER>"));
    assert!(help.contains("<PROPOSAL>"));
    assert!(!help.contains("--key"));
}
