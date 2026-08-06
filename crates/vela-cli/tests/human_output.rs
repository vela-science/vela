//! `--json` must change something.
//!
//! `show`, `why` and `log` all advertised `--json` as "Output stable JSON for
//! programmatic callers", implying a human default, and all three printed the
//! same pretty JSON either way. `log` did not even read its own flag. So the
//! verb whose entire purpose is answering "why does this stand" to a person
//! answered with a few hundred lines of record, including the compaction
//! predecessor archive.
//!
//! These tests assert only the property that was violated: a human asking a
//! read verb a question does not get a serialized object back. What the
//! rendering says is a matter of taste and will change; that it is not JSON is
//! the contract.

use std::path::Path;
use std::process::Command;

mod support;
use support::EphemeralAgent;

/* HOME is isolated to the temp directory. `vela init` installs a local trust
anchor under $HOME/.vela/trust, and a test that used the real HOME would
write into the developer's own trust store and then fail on the second run
refusing to replace what the first left behind. */
fn run(cwd: &Path, socket: Option<&Path>, args: &[&str]) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(cwd)
        .args(args)
        .env("HOME", cwd.join("home"))
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0");
    /* The agent socket is reached through SSH_AUTH_SOCK, as every other
    integration test in this crate does; pointing it at a missing path is
    what makes an unsigned run fail loudly instead of picking up the
    developer's own agent. */
    match socket {
        Some(socket) => command.env("SSH_AUTH_SOCK", socket),
        None => command.env("SSH_AUTH_SOCK", cwd.join("missing-ssh-agent.sock")),
    };
    let output = command.output().expect("vela must run");
    assert!(
        output.status.success(),
        "vela {args:?} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("vela output must be UTF-8")
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

fn initialized_frontier(temporary: &Path, agent: &EphemeralAgent) -> (String, RemoveOnDrop) {
    std::fs::create_dir_all(temporary.join("home")).expect("isolated home");
    let frontier = temporary.join("frontier");
    let text = frontier.to_string_lossy().into_owned();
    let initialized = run(
        temporary,
        Some(agent.socket()),
        &[
            "init",
            &text,
            "--name",
            &format!(
                "Human output fixture {}",
                temporary
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("unique")
            ),
            "--scope",
            "Exercise the human rendering of the read verbs.",
            "--json",
        ],
    );
    let anchor = serde_json::from_str::<serde_json::Value>(initialized.trim())
        .ok()
        .and_then(|value| {
            value["authority"]["local_trust"]["anchor_path"]
                .as_str()
                .map(std::path::PathBuf::from)
        })
        .expect("init must report the trust anchor it installed");
    (text, RemoveOnDrop(anchor))
}

/* One test, not two: each needs an initialized Frontier, and two ephemeral
signing agents starting concurrently in the same process race each other. */
#[test]
fn json_changes_what_a_read_verb_prints() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela human output test");
    let (frontier, _anchor) = initialized_frontier(temporary.path(), &agent);

    for verb in [
        vec!["log", frontier.as_str(), "--limit", "5"],
        vec!["status", frontier.as_str()],
        vec!["claims", frontier.as_str()],
    ] {
        let human = run(temporary.path(), Some(agent.socket()), &verb);
        let name = verb[0];
        assert!(
            serde_json::from_str::<serde_json::Value>(human.trim()).is_err(),
            "`vela {name}` without --json returned a JSON document:\n{human}"
        );
        assert!(
            !human.trim_start().starts_with('{'),
            "`vela {name}` without --json opened with a serialized object:\n{human}"
        );

        let mut asked = verb.clone();
        asked.push("--json");
        let json = run(temporary.path(), Some(agent.socket()), &asked);
        let parsed: serde_json::Value = serde_json::from_str(json.trim())
            .unwrap_or_else(|error| panic!("`vela {name} --json` must emit JSON: {error}\n{json}"));
        assert_eq!(
            parsed["ok"], true,
            "`vela {name} --json` must carry the ok envelope"
        );
        assert_ne!(
            human.trim(),
            json.trim(),
            "`vela {name}` prints the same bytes with and without --json"
        );
    }
}
