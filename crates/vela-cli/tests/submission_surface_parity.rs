//! Submission-wire parity checks across the terminal and MCP intake surfaces.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const DUPLICATE_KEY_SUBMISSION: &str =
    r#"{"schema":"vela.submission.v1","claim":{"assertion":"first","assert\u0069on":"second"}}"#;

fn vela_bin() -> &'static str {
    env!("CARGO_BIN_EXE_vela")
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(vela_bin())
        .current_dir(dir)
        .args(args)
        .env("HOME", dir)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .output()
        .expect("run vela")
}

fn one_json_object(bytes: &[u8], context: &str) -> serde_json::Value {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .unwrap_or_else(|error| panic!("{context} must be exactly one JSON value: {error}"));
    assert!(value.is_object(), "{context} must be one JSON object");
    value
}

#[test]
fn cli_and_mcp_reject_the_same_raw_duplicate_key_submission() {
    let temp = tempfile::tempdir().unwrap();
    let init = run(
        temp.path(),
        &[
            "init",
            ".",
            "--name",
            "submission-parity",
            "--scope",
            "Exercise Submission parity.",
            "--json",
        ],
    );
    assert!(
        init.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    std::fs::write(temp.path().join("duplicate.json"), DUPLICATE_KEY_SUBMISSION).unwrap();
    let before =
        serde_json::to_value(vela_protocol::repo::load_from_path(temp.path()).unwrap()).unwrap();

    let cli = run(
        temp.path(),
        &["submit", "duplicate.json", "--as", "agent:parity", "--json"],
    );
    assert!(
        !cli.status.success(),
        "CLI must reject duplicate object names"
    );
    let cli_response = one_json_object(&cli.stdout, "CLI response");
    assert_eq!(cli_response["ok"], false);
    assert_eq!(cli_response["changed"], false);
    let cli_message = cli_response["error"]["message"].as_str().unwrap();
    assert!(cli_message.contains("duplicate field `assertion`"));
    let after_cli =
        serde_json::to_value(vela_protocol::repo::load_from_path(temp.path()).unwrap()).unwrap();
    assert_eq!(
        after_cli, before,
        "CLI parse failure must leave state unchanged"
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "attempt",
            "arguments": {
                "frontier_path": ".",
                "action": "submit",
                "agent_actor": "agent:parity",
                "submission": DUPLICATE_KEY_SUBMISSION,
            }
        }
    });
    let mut server = Command::new(vela_bin())
        .current_dir(temp.path())
        .args(["serve", ".", "--profile", "draft"])
        .env("HOME", temp.path())
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start MCP server");
    writeln!(server.stdin.as_mut().unwrap(), "{request}").unwrap();
    drop(server.stdin.take());
    let mcp = server.wait_with_output().expect("wait for MCP server");
    assert!(
        mcp.status.success(),
        "MCP server failed: {}",
        String::from_utf8_lossy(&mcp.stderr)
    );
    let rpc = one_json_object(&mcp.stdout, "MCP JSON-RPC response");
    assert_eq!(rpc["result"]["isError"], true);
    let envelope: serde_json::Value =
        serde_json::from_str(rpc["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(envelope["tool"], "attempt");
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["error"]["kind"], "INVALID_ARG");
    assert_eq!(
        envelope["error"]["message"].as_str().unwrap(),
        cli_message,
        "CLI and MCP must feed the same raw bytes to SubmissionV1::parse"
    );
    let after_mcp =
        serde_json::to_value(vela_protocol::repo::load_from_path(temp.path()).unwrap()).unwrap();
    assert_eq!(
        after_mcp, before,
        "MCP parse failure must leave state unchanged"
    );
}
