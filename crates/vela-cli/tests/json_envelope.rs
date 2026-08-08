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

/* `vela init` installs a local trust anchor under the OS ACCOUNT home, resolved
through geteuid/getpwuid_r, which deliberately ignores `$HOME` — there is a
test upstream asserting a hostile HOME cannot redirect it. So setting HOME to
a temp directory does not keep a test out of the developer's real trust
store, and this suite spent an evening quietly filling it. The anchor path
comes back in init's own JSON; delete exactly that file and nothing else. */
struct RemoveOnDrop(std::path::PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn every_json_read_carries_the_envelope() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir_all(temporary.path().join("home")).expect("isolated home");
    let agent = EphemeralAgent::start(temporary.path(), "vela json envelope test");
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();

    let (initialized, init_out) = run(
        temporary.path(),
        agent.socket(),
        &[
            "init",
            &repository_path_text,
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
    assert!(initialized, "the fixture repository must initialize");
    let _anchor = RemoveOnDrop(
        serde_json::from_str::<serde_json::Value>(init_out.trim())
            .ok()
            .and_then(|value| {
                value["authority"]["local_trust"]["anchor_path"]
                    .as_str()
                    .map(std::path::PathBuf::from)
            })
            .expect("init must report the trust anchor it installed"),
    );

    /* Only verbs a fresh repository can answer. `show` and `why` need a retained
    object, and a repository with no Claims has none; asserting them here would
    test the error envelope instead, which is a different contract. */
    for args in [
        vec!["status", repository_path_text.as_str()],
        vec!["next", repository_path_text.as_str()],
        vec!["log", repository_path_text.as_str(), "--limit", "5"],
        vec!["replay", repository_path_text.as_str()],
        vec!["reproduce", repository_path_text.as_str()],
        vec!["review", "inbox", repository_path_text.as_str()],
        vec!["review", "list", repository_path_text.as_str()],
        vec!["claims", repository_path_text.as_str()],
    ] {
        let mut asked = args.clone();
        asked.push("--json");
        let (success, out) = run(temporary.path(), agent.socket(), &asked);
        let verb = args.join(" ");

        let parsed: serde_json::Value = serde_json::from_str(out.trim())
            .unwrap_or_else(|error| panic!("`vela {verb} --json` must emit JSON: {error}\n{out}"));

        /* The contract covers EVERY outcome, not only success — `reproduce` on
        a repository with no witnesses legitimately fails, and that failure
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
