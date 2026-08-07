//! The words the protocol fixes, and the one word it fixes on the wire.
//!
//! TERMINOLOGY.md bans "any unqualified use of `verified`, `valid`, `approved`,
//! or `complete`" in product wording, and prescribes the exact two sentences a
//! successful Submission reports. `status` and `replay` broke the first rule
//! and `submit` never printed the second, so this asserts both properties
//! rather than the particular replacement prose: the replacement is a matter of
//! taste and will change; that these words stay out of it is the contract.
//!
//! One exception is deliberate and asserted here too. `integrity.replay` in
//! `vela.status.v4` is a wire token, not prose: vela-web pins it as
//! `z.literal("verified")`. Retiring it is a coordinated schema change, so a
//! later prose sweep must not quietly take it with the others.
//!
//! `command` is asserted for the same reason. `review withdraw` named itself
//! `proposal.withdraw` when it succeeded and `proposal withdraw` when it
//! failed, so a caller keying on the field saw two names for one invocation and
//! neither was a verb the CLI accepts.
//!
//! The retired-vocabulary half is here because it drifted invisibly. ADR 0039
//! made the Repository the authority boundary and left the Frontier derived and
//! identifier-free, and v0.967.0 took `vfr_` and `frontier_id` to zero — but
//! nothing was reading the help output, so `vela help advanced`, thirteen
//! `--help` bodies and about thirty runtime error strings went on naming a
//! Frontier where they meant a Repository. This walks the whole help tree the
//! binary actually prints and both sides of the error surface, rather than a
//! list of the places that were wrong once.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Output};

mod support;
use support::EphemeralAgent;

/// TERMINOLOGY.md, "Product wording": these are banned unqualified, so the test
/// looks for them as whole words and lets a qualified compound through.
const BANNED_UNQUALIFIED: [&str; 4] = ["verified", "valid", "approved", "complete"];

/// The word ADR 0039 retired from the product surface.
///
/// The CLI only ever addresses a Repository: every verb takes `--repo <path>`,
/// and a Frontier is a query with no identifier and no directory, so there is
/// nothing on this surface a Frontier could name. Compounds count — the drift
/// arrived as `Frontier-relative`, `frontier-owned` and `no frontier found`,
/// not as the bare noun — so this is a substring test, not a word test.
const RETIRED_ON_THE_PRODUCT_SURFACE: &str = "frontier";

fn assert_vocabulary_retired(surface: &str, rendered: &str) {
    assert!(
        !rendered
            .to_ascii_lowercase()
            .contains(RETIRED_ON_THE_PRODUCT_SURFACE),
        "{surface} still says Frontier where ADR 0039 means Repository:\n{rendered}"
    );
}

fn run(cwd: &Path, home: &Path, socket: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(cwd)
        .args(args)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env("SSH_AUTH_SOCK", socket)
        .output()
        .expect("run vela")
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "vela exited {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).expect("vela output must be UTF-8")
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_str(String::from_utf8_lossy(&output.stdout).trim())
        .expect("vela --json must emit one JSON object")
}

fn assert_no_banned_word(verb: &str, rendered: &str) {
    for word in rendered.split(|character: char| !character.is_ascii_alphanumeric()) {
        let word = word.to_ascii_lowercase();
        assert!(
            !BANNED_UNQUALIFIED.contains(&word.as_str()),
            "`vela {verb}` prints the unqualified word {word:?}, which TERMINOLOGY.md bans:\n{rendered}"
        );
    }
}

fn configure_git_identity(frontier: &Path) {
    for (key, value) in [
        ("user.name", "Vela Test"),
        ("user.email", "vela@example.invalid"),
    ] {
        let configured = Command::new("git")
            .current_dir(frontier)
            .args(["config", key, value])
            .status()
            .expect("configure test Git identity");
        assert!(configured.success());
    }
}

/* One test, not four: each needs an initialized repository, and two ephemeral
signing agents starting concurrently in the same process race each other. */
#[test]
fn the_cli_speaks_the_vocabulary_the_protocol_fixes() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela wording contract test");
    let home = temporary.path().join("home");
    std::fs::create_dir_all(&home).expect("isolated home");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();
    let socket = agent.socket();

    /* The repository id is derived from name, scope, and key, and the authority
    trust anchor it installs is keyed by that id in the operating-system account
    home, which no environment variable can redirect. A fixed name would make a
    second run collide with the anchor the first one left. */
    let name = format!(
        "Wording contract fixture {}",
        temporary
            .path()
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("unique")
    );
    let initialized = run(
        temporary.path(),
        &home,
        socket,
        &[
            "init",
            &frontier_text,
            "--name",
            &name,
            "--scope",
            "Exercise the wording TERMINOLOGY.md fixes.",
            "--json",
        ],
    );
    let _anchor =
        support::RemoveAnchorOnDrop::from_init_json(&String::from_utf8_lossy(&initialized.stdout));
    configure_git_identity(&frontier);

    for verb in ["status", "replay"] {
        let rendered = stdout(&run(
            temporary.path(),
            &home,
            socket,
            &[verb, &frontier_text],
        ));
        assert_no_banned_word(verb, &rendered);
    }

    let status = json(&run(
        temporary.path(),
        &home,
        socket,
        &["status", &frontier_text, "--json"],
    ));
    assert_eq!(
        status["integrity"]["replay"], "verified",
        "vela.status.v4 carries `verified` as a wire token; vela-web pins it as z.literal(\"verified\"), so it moves only with a schema bump"
    );

    std::fs::create_dir_all(frontier.join("artifacts")).expect("artifacts directory");
    std::fs::write(
        frontier.join("artifacts/note.json"),
        b"{\"note\":\"wording fixture\"}\n",
    )
    .expect("fixture artifact");
    let submit = [
        "submit",
        "--repo",
        &frontier_text,
        "--claim",
        "Exact bounded fixture claim.",
        "--type",
        "theoretical",
        "--replayability",
        "exact",
        "--artifact",
        "artifacts/note.json:source-diff",
        "--caveat",
        "Fixture only.",
        "--as",
        "agent:fixture",
    ];
    let submitted = stdout(&run(temporary.path(), &home, socket, &submit));
    assert!(
        submitted.contains("Submission retained; review required."),
        "submit must report what a Submission is, in TERMINOLOGY.md's words:\n{submitted}"
    );
    assert!(
        submitted.contains("Accepted scientific state changed: no."),
        "submit must report what it did not change, in TERMINOLOGY.md's words:\n{submitted}"
    );

    let mut submit_json = submit.to_vec();
    submit_json.push("--json");
    submit_json[4] = "Second exact bounded fixture claim.";
    let second = json(&run(temporary.path(), &home, socket, &submit_json));
    assert_eq!(second["ok"], true, "second submission failed: {second}");
    let proposal = second["proposal_id"].as_str().expect("proposal id");

    let withdrawn = json(&run(
        temporary.path(),
        &home,
        socket,
        &[
            "review",
            "withdraw",
            &frontier_text,
            proposal,
            "--as",
            "agent:fixture",
            "--reason",
            "Fixture withdrawal.",
            "--json",
        ],
    ));
    assert_eq!(withdrawn["ok"], true, "withdrawal failed: {withdrawn}");
    let refused = json(&run(
        temporary.path(),
        &home,
        socket,
        &[
            "review",
            "withdraw",
            &frontier_text,
            &format!("vpr_{}", "0".repeat(16)),
            "--as",
            "agent:fixture",
            "--reason",
            "Fixture withdrawal.",
            "--json",
        ],
    ));
    assert_eq!(refused["ok"], false, "unknown Proposal was withdrawn");
    assert_eq!(
        withdrawn["command"], refused["command"],
        "`command` must not change with the outcome"
    );
    assert_eq!(
        withdrawn["command"], "review.withdraw",
        "`command` must name the verb the CLI accepts"
    );
}

/// Run the binary with no repository, agent, or home in play.
///
/// Help is write-free and the error paths under test fail before they reach
/// anything, so neither needs the fixture the wording test above builds.
fn plain(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(cwd)
        .args(args)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .output()
        .expect("run vela")
}

fn help_body(cwd: &Path, path: &[&str]) -> String {
    let mut args = path.to_vec();
    args.push("--help");
    let output = plain(cwd, &args);
    assert!(
        output.status.success(),
        "`vela {} --help` exited {:?}",
        path.join(" "),
        output.status.code()
    );
    String::from_utf8(output.stdout).expect("help must be UTF-8")
}

/// The entries of a clap `Commands:` block, which is how a verb declares its
/// subverbs. `help` is clap's own and describes nothing.
fn subcommands(rendered: &str) -> Vec<String> {
    let Some((_, after)) = rendered.split_once("Commands:\n") else {
        return Vec::new();
    };
    after
        .lines()
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let (name, rest) = line.strip_prefix("  ")?.split_once(' ')?;
            (!name.is_empty() && rest.starts_with(' ')).then(|| name.to_string())
        })
        .filter(|name| name != "help")
        .collect()
}

/// The verbs the binary publishes, read out of `vela help advanced`.
///
/// Naming them here would rot the moment a verb is added, which is exactly how
/// the last drift survived: nothing read the help at all. The advanced grid is
/// already held equal to `Cli::command()` by `cli/surface.rs`, so reading it is
/// reading the parser through one chain rather than keeping a second list.
fn published_verbs(cwd: &Path) -> BTreeSet<String> {
    let advanced = String::from_utf8(plain(cwd, &["help", "advanced"]).stdout)
        .expect("advanced help must be UTF-8");
    let verbs: BTreeSet<String> = advanced
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.strip_prefix("  ")?.split_once(' ')?;
            (!name.is_empty() && rest.starts_with(' ')).then(|| name.to_string())
        })
        .collect();
    assert!(
        verbs.contains("init") && verbs.contains("review") && verbs.len() > 10,
        "`vela help advanced` no longer parses as a verb grid; this test would \
         silently cover nothing:\n{advanced}"
    );
    verbs
}

/// Every help body the binary prints, from both hand-set grids down through
/// each verb and subverb, may not name a Frontier.
///
/// The bodies checked here are `vela help`, `vela help advanced`, `vela --help`
/// and one `--help` per node of the parser tree — the `after_long_help` blocks
/// in `cli/help_text.rs`, the `about` and `///` strings clap renders from
/// `command_spec.rs`, and every flag's help. None of it was read by any test
/// before, which is why `--frontier` survived in the quick start and
/// `HELP_FRONTIER_BEFORE_OBJECT` survived on four verbs.
#[test]
fn no_help_body_names_a_frontier() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let cwd = temporary.path();

    for grid in [vec!["help"], vec!["help", "advanced"]] {
        let rendered = String::from_utf8(plain(cwd, &grid).stdout).expect("grid must be UTF-8");
        assert_vocabulary_retired(&format!("`vela {}`", grid.join(" ")), &rendered);
    }

    let mut pending: Vec<Vec<String>> = vec![Vec::new()];
    let mut seen = 0usize;
    while let Some(path) = pending.pop() {
        let borrowed: Vec<&str> = path.iter().map(String::as_str).collect();
        let rendered = help_body(cwd, &borrowed);
        assert_vocabulary_retired(&format!("`vela {} --help`", path.join(" ")), &rendered);
        seen += 1;

        let children = if path.is_empty() {
            /* The root help is the hand-set product grid, not a clap
            `Commands:` block, so the verb list comes from the advanced
            reference the parser is already held to. */
            published_verbs(cwd).into_iter().collect()
        } else {
            subcommands(&rendered)
        };
        for child in children {
            let mut next = path.clone();
            next.push(child);
            pending.push(next);
        }
    }
    assert!(
        seen > 20,
        "the help walk reached only {seen} bodies; the parser tree has more"
    );
}

/// The failure surface, on both streams.
///
/// `vela show <id>` outside any repository printed "no frontier found from …"
/// long after the identifier was gone, because the only wording assertions ran
/// against successful `status` and `replay` output. A user meets these lines
/// more often than the help.
#[test]
fn no_failure_message_names_a_frontier() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let cwd = temporary.path();
    let missing = cwd.join("absent-repository");
    let missing = missing.to_string_lossy().into_owned();
    let claim = format!("vcl_{}", "0".repeat(16));

    for args in [
        vec!["status", missing.as_str()],
        vec!["replay", missing.as_str()],
        vec!["claims", missing.as_str()],
        vec!["status"],
        vec!["show", claim.as_str()],
        vec!["why", claim.as_str()],
        vec!["review", "inbox"],
        vec!["log"],
    ] {
        let output = plain(cwd, &args);
        assert!(
            !output.status.success(),
            "`vela {}` was expected to fail outside a repository",
            args.join(" ")
        );
        let rendered = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_vocabulary_retired(&format!("`vela {}`", args.join(" ")), &rendered);
    }
}
