//! Pre-lifecycle validation of the authoritative Review Method parser and its
//! exact Verification bindings.

use std::path::Path;
use std::process::{Command, Output};

fn run(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(cwd)
        .args(args)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .output()
        .expect("run vela verification check")
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "decode JSON: {error}\nstatus={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn canonical_review_method_check_is_non_mutating_and_binding_exact() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let method = temporary.path().join("method.json");
    std::fs::write(
        &method,
        include_bytes!("../../../examples/review-methods/computational-formal.json"),
    )
    .expect("write canonical method");
    let method_text = method.to_string_lossy().into_owned();
    let nonclaim = "Statement faithfulness, empirical applicability, novelty, significance, scientific acceptance, or Standing.";
    let args = [
        "verification",
        "check",
        &method_text,
        "--profile",
        "computational-formal-verification-v1",
        "--property",
        "computational_or_formal_check",
        "--as",
        "verifier:exact-checker",
        "--does-not-establish",
        nonclaim,
        "--json",
    ];

    let before = std::fs::read(&method).expect("method before check");
    let checked = run(temporary.path(), &args);
    assert!(
        checked.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );
    let checked = json(&checked);
    assert_eq!(checked["schema"], "vela.review-method-validation.v1");
    assert_eq!(checked["command"], "verification.check");
    assert_eq!(checked["changed"], false);
    assert_eq!(checked["authority_effect"], "none");
    assert_eq!(checked["standing_effect"], "none");
    assert_eq!(checked["bindings"]["matched"], true);
    assert_eq!(
        std::fs::read(&method).expect("method after check"),
        before,
        "validation must not change its input"
    );

    let mismatch = run(
        temporary.path(),
        &[
            "verification",
            "check",
            &method_text,
            "--profile",
            "wrong-profile",
            "--property",
            "computational_or_formal_check",
            "--as",
            "verifier:exact-checker",
            "--does-not-establish",
            nonclaim,
            "--json",
        ],
    );
    assert_eq!(mismatch.status.code(), Some(1));
    let mismatch = json(&mismatch);
    assert_eq!(mismatch["schema"], "vela.error.v1");
    assert_eq!(mismatch["error"]["code"], "review_method_binding_mismatch");
    assert_eq!(mismatch["changed"], false);
    assert_eq!(mismatch["retained"]["transaction_marker"], false);
    assert_eq!(
        std::fs::read(&method).expect("method after binding mismatch"),
        before,
        "binding refusal must not change its input"
    );

    std::fs::write(&method, br#"{"schema":"vela.review-method.v1"}"#)
        .expect("write invalid declared method");
    let malformed_before = std::fs::read(&method).expect("invalid method before check");
    let invalid = run(temporary.path(), &args);
    assert_eq!(invalid.status.code(), Some(1));
    let invalid = json(&invalid);
    assert_eq!(invalid["schema"], "vela.error.v1");
    assert_eq!(invalid["error"]["code"], "review_method_invalid");
    assert_eq!(invalid["changed"], false);
    assert_eq!(invalid["retained"]["transaction_marker"], false);
    assert_eq!(
        std::fs::read(&method).expect("invalid method after check"),
        malformed_before,
        "parse refusal must not change its input"
    );
}
