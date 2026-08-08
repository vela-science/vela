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

/// The other retired names, from `docs/TERMINOLOGY.md`'s "Retired names" table.
///
/// Only the multi-word ones are here, and the omissions are the point. `Finding`
/// and `Attempt` are ordinary English words that this surface uses in their
/// ordinary sense — `--source-attempt` carries `provenance.source_attempt`,
/// which is exactly the "workbench's own run identity, as provenance" the table
/// says to use instead — so a substring test on them would be a test against
/// English. `Frontier Commit` and `Frontier map` are already caught above.
/// `Review Packet` is the one the Frontier check cannot see, and the binary was
/// printing it in three places while `docs/ECOSYSTEM.md` listed it as retired
/// and not to be reintroduced.
const RETIRED_OBJECT_NAMES: [&str; 1] = ["review packet"];

fn assert_vocabulary_retired(surface: &str, rendered: &str) {
    let lowered = rendered.to_ascii_lowercase();
    assert!(
        !lowered.contains(RETIRED_ON_THE_PRODUCT_SURFACE),
        "{surface} still says Frontier where ADR 0039 means Repository:\n{rendered}"
    );
    for retired in RETIRED_OBJECT_NAMES {
        assert!(
            !lowered.contains(retired),
            "{surface} names {retired:?}, which docs/TERMINOLOGY.md retired:\n{rendered}"
        );
    }
}

/// The retired word as a JSON *key*, anywhere in a document at any depth.
///
/// The prose check above cannot be run against `--json`: a repository whose
/// directory a caller named `frontier` puts the substring in every payload that
/// echoes the path, and refusing that would be refusing the caller's own
/// filename. So keys are checked instead, which is the half a consumer actually
/// binds to.
///
/// This exists because three payloads kept the key after `replay` gave it up.
/// `vela.review-decision`, `vela.authority-trust-pin-result` and
/// `vela.authority-initialization-result` each carried `"repository"` holding a
/// filesystem path, beside a `repository_id` naming the thing at that path —
/// one document with two vocabularies, in the only place where the word had a
/// consumer. Each was renamed to `repository_path` and bumped a version,
/// because a published key moves with a version rather than in a sweep.
fn assert_no_retired_key(surface: &str, document: &serde_json::Value) {
    match document {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                assert!(
                    !key.to_ascii_lowercase()
                        .contains(RETIRED_ON_THE_PRODUCT_SURFACE),
                    "{surface} publishes `{key}` as a key; ADR 0039 retired that noun and a caller binds to this:\n{document}"
                );
                assert_no_retired_key(surface, value);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                assert_no_retired_key(surface, item);
            }
        }
        _ => {}
    }
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

fn configure_git_identity(repository_path: &Path) {
    for (key, value) in [
        ("user.name", "Vela Test"),
        ("user.email", "vela@example.invalid"),
    ] {
        let configured = Command::new("git")
            .current_dir(repository_path)
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
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();
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
            &repository_path_text,
            "--name",
            &name,
            "--scope",
            "Exercise the wording TERMINOLOGY.md fixes.",
            "--json",
        ],
    );
    let _anchor =
        support::RemoveAnchorOnDrop::from_init_json(&String::from_utf8_lossy(&initialized.stdout));
    configure_git_identity(&repository_path);

    for verb in ["status", "replay"] {
        let rendered = stdout(&run(
            temporary.path(),
            &home,
            socket,
            &[verb, &repository_path_text],
        ));
        assert_no_banned_word(verb, &rendered);
    }

    let status = json(&run(
        temporary.path(),
        &home,
        socket,
        &["status", &repository_path_text, "--json"],
    ));
    assert_eq!(
        status["integrity"]["replay"], "verified",
        "vela.status.v4 carries `verified` as a wire token; vela-web pins it as z.literal(\"verified\"), so it moves only with a schema bump"
    );

    /* Two JSON tokens spelled the retired word until they were bumped, and
    each moved with a version rather than in a sweep for the same reason
    `integrity.replay` has not moved at all: a caller keys on them. `repository_path`
    was a key of `vela.repository-verification.v2` and is `repository_path` in
    `.v3`; `accepted_frontier` was a scope value of
    `vela.reproduction-summary.v1` and is `accepted_repository` in `.v2`, which
    `docs/VERIFICATION.md` documents by name. Both halves are asserted — the
    new token present and the retired one absent — so a revert fails here
    instead of shipping one document with two names for one thing. */
    let replayed = json(&run(
        temporary.path(),
        &home,
        socket,
        &["replay", &repository_path_text, "--json"],
    ));
    assert_eq!(replayed["schema"], "vela.repository-verification.v3");
    assert!(
        replayed["repository_path"].is_string(),
        "`repository_path` is the published path key of vela.repository-verification.v3; dropping it is a v4 bump, not a prose sweep:\n{replayed}"
    );
    assert!(
        replayed.get("frontier").is_none(),
        "v3 retired the `repository_path` key; a document carrying both names one path twice:\n{replayed}"
    );

    /* The reproduction scope, on both surfaces. One witness from the
    checked-in corpus gives `reproduce` something to re-run; without it the
    command fails before it reaches the token, which is how the human half of
    this contract went unasserted while a comment claimed it. */
    let witnesses = repository_path.join("witnesses");
    std::fs::create_dir_all(&witnesses).expect("witness directory");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../vela-verify/corpus/valid/smoke/sidon.witness.json"),
        witnesses.join("sidon.witness.json"),
    )
    .expect("copy one corpus witness into the fixture repository");
    let reproduced = json(&run(
        temporary.path(),
        &home,
        socket,
        &["reproduce", &repository_path_text, "--json"],
    ));
    assert_eq!(reproduced["schema"], "vela.reproduction-summary.v2");
    assert_eq!(
        reproduced["scope"], "accepted_repository",
        "the repository scope is `accepted_repository` in vela.reproduction-summary.v2:\n{reproduced}"
    );
    let reproduced_text = stdout(&run(
        temporary.path(),
        &home,
        socket,
        &["reproduce", &repository_path_text],
    ));
    /* Not `assert_vocabulary_retired`: this fixture's directory is literally
    named `repository_path`, and `reproduce` echoes the path it was given. The token
    is what is under test, so the token is what is asserted. */
    assert!(
        reproduced_text.contains("scope: accepted_repository"),
        "`vela reproduce` prints its scope token verbatim, so the human surface moves with the schema:\n{reproduced_text}"
    );
    assert!(
        !reproduced_text.contains("accepted_frontier"),
        "the retired scope token is back on the human surface:\n{reproduced_text}"
    );

    std::fs::create_dir_all(repository_path.join("artifacts")).expect("artifacts directory");
    std::fs::write(
        repository_path.join("artifacts/note.json"),
        b"{\"note\":\"wording fixture\"}\n",
    )
    .expect("fixture artifact");
    let submit = [
        "submit",
        "--repo",
        &repository_path_text,
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
            &repository_path_text,
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
            &repository_path_text,
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

    /* Every document this test produced, swept for the retired noun as a key.
    Asserting `repository_path` absent from `replay` alone was a fix for one payload
    where the rule wanted all of them, and three others kept the key for a
    release after that assertion was written.

    Six is not every `--json` payload the binary can emit. `vela init` renders
    prose in this fixture, and `authority trust pin` and `review accept` need
    setup this test does not build; those carry the assertion in
    `review_acceptance.rs` and in their own version bumps instead. This is a
    floor that grows for free — a payload a later assertion adds to this test
    joins the sweep by adding one line below. */
    for (surface, document) in [
        ("vela status --json", &status),
        ("vela replay --json", &replayed),
        ("vela reproduce --json", &reproduced),
        ("vela submit --json", &second),
        ("vela review withdraw --json", &withdrawn),
        ("vela review withdraw --json (refused)", &refused),
    ] {
        assert_no_retired_key(surface, document);
    }
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

/// The verbs `docs/ECOSYSTEM.md` §8 names, and the count it states beside them.
///
/// The layering diagram is the fourth place the verb list is written down and
/// the only one nothing read. `cli/surface.rs` holds both printed grids to
/// `Cli::command()` and `docs/CLI.md` to the grids; this document sat outside
/// that chain and said "15 verbs" for as long as there had been sixteen, having
/// missed `correction` entirely.
fn layering_block_verbs() -> (usize, BTreeSet<String>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/ECOSYSTEM.md");
    let document =
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read ECOSYSTEM.md: {error}"));
    let (before, after) = document
        .split_once(" verbs: ")
        .expect("docs/ECOSYSTEM.md §8 no longer states a verb count");
    let stated: usize = before
        .split_whitespace()
        .next_back()
        .expect("the verb count has no number before it")
        .parse()
        .expect("the verb count is not a number");
    let listed = after
        .split_once("\n  readers")
        .expect("docs/ECOSYSTEM.md §8 no longer closes the operator row with the readers row")
        .0;
    (
        stated,
        listed.split_whitespace().map(str::to_string).collect(),
    )
}

/// §8's operator row is the surface the binary actually has.
#[test]
fn the_layering_diagram_names_the_verbs_the_binary_publishes() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (stated, listed) = layering_block_verbs();
    assert_eq!(
        listed,
        published_verbs(temporary.path()),
        "docs/ECOSYSTEM.md §8 and `vela help advanced` disagree about which verbs exist"
    );
    assert_eq!(
        stated,
        listed.len(),
        "docs/ECOSYSTEM.md §8 states {stated} verbs and lists {}",
        listed.len()
    );
}

/// Every help body the binary prints, from both hand-set grids down through
/// each verb and subverb, may not name a Frontier.
///
/// The bodies checked here are `vela help`, `vela help advanced`, `vela --help`
/// and one `--help` per node of the parser tree — the `after_long_help` blocks
/// in `cli/help_text.rs`, the `about` and `///` strings clap renders from
/// `command_spec.rs`, and every flag's help. None of it was read by any test
/// before, which is why `--repository` survived in the quick start and
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
/// `vela show <id>` outside any repository printed "no repository found from …"
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

/// The half of the error surface that is a contract.
///
/// This file says at the top that the prose "will change; that these words stay
/// out of it is the contract". A caller still has to tell one failure from
/// another, and until 2026-08-07 the only thing offering to do that was the
/// prose. So `error.code` is asserted the way `command` is above: that the
/// failing invocation names itself, that the name is one the binary declares,
/// and that it does not move with the wording.
#[test]
fn a_coded_failure_names_itself_from_the_published_list() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let cwd = temporary.path();
    let missing = cwd.join("absent-repository");
    let missing = missing.to_string_lossy().into_owned();

    for (args, expected) in [
        (
            vec!["replay", missing.as_str(), "--json"],
            "repository_missing",
        ),
        (
            vec!["claims", missing.as_str(), "--json"],
            "repository_missing",
        ),
    ] {
        let output = plain(cwd, &args);
        assert!(
            !output.status.success(),
            "`vela {}` was expected to fail",
            args.join(" ")
        );
        let rendered = json(&output);
        assert_eq!(
            rendered["error"]["code"],
            expected,
            "`vela {}` must name which refusal this is:\n{rendered}",
            args.join(" ")
        );
    }

    /* Every emitted code is one the binary published. A code invented at a call
    site is worse than no code: a caller branches on it and the next release
    spells it differently, which is the failure mode `message` already has
    and the whole reason this field exists. */
    let declared: BTreeSet<&str> = vela_cli::ERROR_CODES.iter().copied().collect();
    assert!(
        declared.contains("repository_missing"),
        "the assertions above must key on a declared code"
    );

    /* A null code is the honest answer where the kind is the whole story, and
    it must stay a present key so a caller reads one shape either way. */
    let usage = plain(cwd, &["submit", "--json"]);
    assert!(
        !usage.status.success(),
        "`vela submit` with no input must fail"
    );
    let rendered = json(&usage);
    assert!(
        rendered["error"].get("code").is_some(),
        "every failure must carry the key, present or null:\n{rendered}"
    );
    if let Some(code) = rendered["error"]["code"].as_str() {
        assert!(
            declared.contains(code),
            "`vela submit` emitted the undeclared code {code:?}"
        );
    }
}
