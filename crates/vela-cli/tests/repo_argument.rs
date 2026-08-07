//! The repository argument has one shape and one resolution behaviour.
//!
//! It used to have four of each: a required leading positional on `show`,
//! `why`, `review *` and `verification *`; an optional one on `status`, `next`
//! and `log`; `default_value = "."` on `authority trust pin`; and a
//! `--repo` flag on `start` and `submit`. A reader who learned `vela
//! status` and then typed `vela show vcl_…` had the object id bound to the
//! repository slot and was told the *object id* was missing.
//!
//! These tests assert the convention itself rather than any message: every
//! documented spelling still binds the same way, the short spelling produces
//! the identical payload, `--repo` is accepted everywhere, and a usage
//! error names the argument that is actually absent.

use std::path::Path;
use std::process::{Command, Output};

mod support;
use support::EphemeralAgent;

fn run(cwd: &Path, home: &Path, socket: Option<&Path>, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(cwd)
        .args(args)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0");
    match socket {
        Some(socket) => command.env("SSH_AUTH_SOCK", socket),
        None => command.env("SSH_AUTH_SOCK", cwd.join("missing-ssh-agent.sock")),
    };
    command.output().expect("vela must run")
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "decode Vela JSON: {error}\nstatus={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// One fixture repository holding one pending Proposal, so `show`, `why`,
/// `review show` and `log` all have a real object to name. Two ephemeral
/// signing agents in one process race each other, so this crate keeps one
/// test per file rather than one per assertion.
struct Fixture {
    _temporary: tempfile::TempDir,
    _agent: EphemeralAgent,
    root: std::path::PathBuf,
    home: std::path::PathBuf,
    frontier: std::path::PathBuf,
    submission_id: String,
    proposal_id: String,
    claim_id: String,
}

impl Fixture {
    fn build() -> Self {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().to_path_buf();
        let home = root.join("home");
        std::fs::create_dir_all(&home).expect("isolated home");
        let agent = EphemeralAgent::start(&root, "vela repository argument test");
        let frontier = root.join("frontier");
        let frontier_text = frontier.to_string_lossy().into_owned();

        let initialized = run(
            &root,
            &home,
            Some(agent.socket()),
            &[
                "init",
                &frontier_text,
                "--name",
                &format!(
                    "Repository argument fixture {}",
                    root.file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("unique")
                ),
                "--scope",
                "Exercise one repository-argument convention across the surface.",
                "--json",
            ],
        );
        let _anchor = support::RemoveAnchorOnDrop::from_init_json(&String::from_utf8_lossy(
            &initialized.stdout,
        ));
        assert!(
            initialized.status.success(),
            "fixture init failed: {}",
            stderr(&initialized)
        );

        std::fs::write(
            frontier.join("witness.json"),
            br#"{"schema":"vela.test-witness.v1","observed":true}"#,
        )
        .expect("fixture artifact");

        let submitted = run(
            &frontier,
            &home,
            Some(agent.socket()),
            &[
                "submit",
                "--claim",
                "The fixture bound is exact.",
                "--type",
                "computational",
                "--replayability",
                "exact",
                "--artifact",
                "witness.json:witness",
                "--caveat",
                "Fixture only.",
                "--as",
                "agent:repository-argument-fixture",
                "--json",
            ],
        );
        assert!(
            submitted.status.success(),
            "fixture submit failed: {}",
            stderr(&submitted)
        );
        let submitted = json(&submitted);
        Self {
            root,
            home,
            frontier,
            submission_id: submitted["submission_id"]
                .as_str()
                .expect("submission id")
                .to_string(),
            proposal_id: submitted["proposal_id"]
                .as_str()
                .expect("proposal id")
                .to_string(),
            claim_id: submitted["claim_id"]
                .as_str()
                .expect("claim id")
                .to_string(),
            _agent: agent,
            _temporary: temporary,
        }
    }

    /// Run inside the repository, where discovery has something to find.
    fn inside(&self, args: &[&str]) -> Output {
        run(&self.frontier, &self.home, None, args)
    }

    /// Run outside any repository, where discovery must fail rather than
    /// silently pick up the developer's own tree.
    fn outside(&self, args: &[&str]) -> Output {
        run(&self.root, &self.home, None, args)
    }
}

#[test]
fn one_frontier_convention_across_the_surface() {
    let fixture = Fixture::build();
    let frontier = fixture.frontier.to_string_lossy().into_owned();
    let frontier = frontier.as_str();
    let claim = fixture.claim_id.as_str();
    let submission = fixture.submission_id.as_str();
    let proposal = fixture.proposal_id.as_str();

    /* Every documented spelling, and the short spelling that replaces it.
    Both must return byte-identical payloads: if the two disagree the
    convention is decorative. The `.` form is what docs/CLI.md,
    docs/QUICKSTART.md and docs/AGENT_QUICKSTART.md publish today. */
    for (documented, short) in [
        (vec!["status", ".", "--json"], vec!["status", "--json"]),
        (vec!["next", ".", "--json"], vec!["next", "--json"]),
        (vec!["replay", ".", "--json"], vec!["replay", "--json"]),
        (
            vec!["log", ".", "--limit", "5", "--json"],
            vec!["log", "--limit", "5", "--json"],
        ),
        (
            vec!["log", ".", proposal, "--json"],
            vec!["log", proposal, "--json"],
        ),
        (
            vec!["show", ".", claim, "--json"],
            vec!["show", claim, "--json"],
        ),
        (
            vec!["show", ".", submission, "--json"],
            vec!["show", submission, "--json"],
        ),
        (
            vec!["why", ".", claim, "--json"],
            vec!["why", claim, "--json"],
        ),
        (
            vec!["review", "inbox", ".", "--json"],
            vec!["review", "inbox", "--json"],
        ),
        (
            vec!["review", "list", ".", "--json"],
            vec!["review", "list", "--json"],
        ),
        (
            vec!["review", "show", ".", proposal, "--json"],
            vec!["review", "show", proposal, "--json"],
        ),
    ] {
        let long = fixture.inside(&documented);
        assert!(
            long.status.success(),
            "the documented form `vela {}` stopped working: {}",
            documented.join(" "),
            stderr(&long)
        );
        let brief = fixture.inside(&short);
        assert!(
            brief.status.success(),
            "`vela {}` must resolve the repository by discovery: {}",
            short.join(" "),
            stderr(&brief)
        );
        assert_eq!(
            json(&long),
            json(&brief),
            "`vela {}` and `vela {}` are the same request and must answer alike",
            documented.join(" "),
            short.join(" ")
        );

        /* The third spelling. `--repo` was previously accepted on
        `start` and `submit` only, which is why a user could not carry the
        habit; it now works on every verb that acts on a repository, and it
        works from outside the tree, where discovery has nothing to find. */
        let mut flagged = short.clone();
        flagged.extend_from_slice(&["--repo", frontier]);
        let flagged = fixture.outside(&flagged);
        assert!(
            flagged.status.success(),
            "`vela {} --repo <path>` must work from outside the repository: {}",
            short.join(" "),
            stderr(&flagged)
        );
        assert_eq!(
            json(&long),
            json(&flagged),
            "`--repo` must name the same repository the positional does"
        );
    }

    /* The write verbs bind the same way, and this is as far as the assertion
    may go: both spellings must reach the record parse, which is past frontier
    resolution and before any durable write. */
    std::fs::write(fixture.frontier.join("not-a-record.json"), b"{}")
        .expect("malformed record fixture");
    for args in [
        vec![
            "verification",
            "import",
            ".",
            "not-a-record.json",
            "--as",
            "verifier:binding",
            "--json",
        ],
        vec![
            "verification",
            "import",
            "not-a-record.json",
            "--as",
            "verifier:binding",
            "--json",
        ],
        vec![
            "verification",
            "import",
            "--repo",
            ".",
            "not-a-record.json",
            "--as",
            "verifier:binding",
            "--json",
        ],
    ] {
        let reached = json(&fixture.inside(&args));
        assert!(
            reached["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("not-a-record.json")),
            "`vela {}` must resolve the repository and then read the record: {reached}",
            args.join(" ")
        );
    }

    a_missing_argument_error_names_the_missing_argument(&fixture);
}

/* Called from the one test rather than declared as a second: two ephemeral
signing agents starting concurrently in the same process race each other, so
this crate keeps one fixture per test binary. */
fn a_missing_argument_error_names_the_missing_argument(fixture: &Fixture) {
    let claim = fixture.claim_id.clone();

    /* The whole point of the finding: `vela show <claim>` used to consume the
    object id as the repository and then report the OBJECT as missing. */
    for (args, wanted) in [
        (vec!["show", "--json"], "needs an object id"),
        (vec!["show", ".", "--json"], "needs an object id"),
        (vec!["why", "--json"], "needs a full Claim id"),
        (
            vec!["review", "show", "--json"],
            "needs a Proposal id (vpr_...)",
        ),
        (
            vec!["verification", "import", "--json", "--as", "verifier:x"],
            "needs a signed Verification Record file",
        ),
    ] {
        let refused = fixture.inside(&args);
        assert_eq!(
            refused.status.code(),
            Some(2),
            "`vela {}` must exit 2 (usage): {}",
            args.join(" "),
            stderr(&refused)
        );
        let refused = json(&refused);
        assert_eq!(refused["error"]["kind"], "usage", "args={args:?}");
        assert!(
            refused["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains(wanted)),
            "`vela {}` reported {:?} instead of naming {wanted}",
            args.join(" "),
            refused["error"]["message"]
        );
        /* A usage failure keeps the envelope: the command must still name
        itself so an agent can tell which call went wrong. */
        assert!(refused["command"].is_string(), "args={args:?}");
    }

    /* `status` has no object slot, so an object id there is a real mistake.
    It used to fall through to "repository directory does not exist" with a
    hint pointing at `vela init` — a writing verb offered to repair an
    argument-order error. */
    let misplaced = fixture.inside(&["status", &claim, "--json"]);
    assert_eq!(misplaced.status.code(), Some(2));
    let misplaced = json(&misplaced);
    assert_eq!(misplaced["error"]["kind"], "usage");
    assert!(
        misplaced["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("is an object id")),
        "{misplaced}"
    );
    assert!(
        misplaced["error"]["hint"]
            .as_str()
            .is_some_and(|hint| !hint.contains("vela init")),
        "an argument-order mistake must not be answered with a writing verb: {misplaced}"
    );

    /* Both spellings at once is ambiguous, and guessing would be worse than
    refusing. */
    let twice = fixture.inside(&["why", "--repo", ".", ".", &claim, "--json"]);
    assert_eq!(twice.status.code(), Some(2));
    assert!(
        json(&twice)["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("given twice"))
    );
}
