//! Every `--json` outcome carries `{ok, command, schema}`.
//!
//! ui.rs states this as "the one output contract", and two verbs did not keep
//! it: `review inbox` named neither `ok` nor `command`, and `reproduce` named
//! the command and nothing else. An agent could not test one field to learn
//! whether a call had succeeded, which is the entire point of the envelope.
//!
//! The gap survived because nothing asserted it across the surface — each verb
//! was checked, if at all, against its own expected payload. This test walks the
//! read verbs together, so a new one cannot ship without the envelope and an
//! existing one cannot quietly lose it.

use std::path::Path;
use std::process::Command;

mod support;
use support::EphemeralAgent;

fn run(cwd: &Path, socket: &Path, args: &[&str]) -> (bool, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(cwd)
        .args(args)
        .env("HOME", cwd.join("home"))
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env("SSH_AUTH_SOCK", socket);
    let output = command.output().expect("vela must run");
    (
        output.status.success(),
        String::from_utf8(output.stdout).expect("vela output must be UTF-8"),
    )
}

#[test]
fn every_json_read_carries_the_envelope() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir_all(temporary.path().join("home")).expect("isolated home");
    let agent = EphemeralAgent::start(temporary.path(), "vela json envelope test");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();

    let (initialized, _) = run(
        temporary.path(),
        agent.socket(),
        &[
            "init",
            &frontier_text,
            "--name",
            &format!(
                "Envelope fixture {}",
                temporary
                    .path()
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("unique")
            ),
            "--scope",
            "Exercise the JSON envelope across the read surface.",
            "--json",
        ],
    );
    assert!(initialized, "the fixture frontier must initialize");

    /* Only verbs a fresh Frontier can answer. `show` and `why` need a retained
    object, and a Frontier with no Claims has none; asserting them here would
    test the error envelope instead, which is a different contract. */
    for args in [
        vec!["status", frontier_text.as_str()],
        vec!["next", frontier_text.as_str()],
        vec!["log", frontier_text.as_str(), "--limit", "5"],
        vec!["replay", frontier_text.as_str()],
        vec!["reproduce", frontier_text.as_str()],
        vec!["review", "inbox", frontier_text.as_str()],
        vec!["review", "list", frontier_text.as_str()],
    ] {
        let mut asked = args.clone();
        asked.push("--json");
        let (success, out) = run(temporary.path(), agent.socket(), &asked);
        let verb = args.join(" ");

        let parsed: serde_json::Value = serde_json::from_str(out.trim())
            .unwrap_or_else(|error| panic!("`vela {verb} --json` must emit JSON: {error}\n{out}"));

        /* The contract covers EVERY outcome, not only success — `reproduce` on
        a Frontier with no witnesses legitimately fails, and that failure
        must still be one object an agent can read. `schema` names the
        payload's shape and so belongs to a payload; a failure carries the
        error instead. */
        for key in ["ok", "command"] {
            assert!(
                parsed.get(key).is_some(),
                "`vela {verb} --json` omits `{key}` from the envelope ui.rs promises"
            );
        }
        assert_eq!(
            parsed["ok"],
            success,
            "`vela {verb} --json` exited {} but its envelope says ok={}",
            if success { "0" } else { "non-zero" },
            parsed["ok"]
        );
        if success {
            assert!(
                parsed.get("schema").is_some(),
                "`vela {verb} --json` succeeded without naming its schema"
            );
        } else {
            assert!(
                parsed.get("error").is_some(),
                "`vela {verb} --json` failed without an error object"
            );
        }
    }
}
