//! One authenticated portable Submission, two local Repository Decisions.
//!
//! The fixture proves the Protocol 1 boundary without adding a federation or
//! policy surface: identical producer bytes enter two independently initialized
//! repositories, whose local authorities accept and reject respectively. Each
//! resulting history replays to the same exact root from a clean clone.

#![cfg(all(unix, feature = "test-support"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};
use vela_protocol::review_method::{ReviewMethodV1, ReviewPerformerV1};

mod support;
use support::{
    EphemeralAgent, RemoveAnchorOnDrop, configure_git_identity, run_with_isolated_home as run,
    success_json,
};

const SUBMISSION_ID: &str = "vsb_f1669cdfa498ff85";
const SUBMISSION_ROOT: &str =
    "sha256:f1669cdfa498ff85c162bce6173f04b39cdf7620fb198a19b45f6d932302204a";
const CLAIM_ID: &str = "vcl_cea6cdb3e9fd02fae86886a0edbe51e5c2fe2d5e00dc7f264d4c3de0f9f2c422";
const CLAIM_ROOT: &str = "sha256:e865c5a2aafd459d52d9b1c8a7734104b1e2d8d1c047c5400684f01505f83632";
const PRODUCER: &str = "agent:independent-js";
const REQUIREMENT: &str = "Recompute the result from the exact fixture bytes.";
const VERIFIER: &str = "verifier:portable-divergence";
const ACCEPT_PERFORMER: &str = "agent:portable-divergence-acceptor";
const ACCEPT_SESSION_REF: &str = "fixture:portable-divergence:accept";
const ACCEPT_REASON: &str =
    "The exact synthetic fixture check passed within this Repository's bounded scope.";
const REJECT_PERFORMER: &str = "agent:portable-divergence-rejector";
const REJECT_SESSION_REF: &str = "fixture:portable-divergence:reject";
const REJECT_REASON: &str =
    "This Repository declines to admit the synthetic Claim without its own local check.";
const ACCEPT_DEVICE: &str = "11111111111111111111111111111111";
const REJECT_DEVICE: &str = "22222222222222222222222222222222";

fn git(repository: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repository)
        .args(args)
        .output()
        .expect("run Git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("Git output is UTF-8")
        .trim()
        .to_string()
}

fn run_on_device(
    cwd: &Path,
    socket: Option<&Path>,
    home: &Path,
    device: &str,
    args: &[&str],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(cwd)
        .args(args)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env("VELA_TEST_DEVICE_IDENTIFIER", device)
        .env_remove("VELA_AGENT_KEY_HEX");
    match socket {
        Some(socket) => command.env("SSH_AUTH_SOCK", socket),
        None => command.env("SSH_AUTH_SOCK", cwd.join("missing-ssh-agent.sock")),
    };
    command.output().expect("run vela on synthetic device")
}

fn repository_manifest(repository: &Path) -> Value {
    serde_json::from_slice(
        &std::fs::read(repository.join(".vela/repository.json")).expect("read repository manifest"),
    )
    .expect("parse repository manifest")
}

fn authority_record_count(repository: &Path) -> usize {
    std::fs::read_dir(repository.join(".vela/authority/records"))
        .expect("read authority records")
        .count()
}

fn assert_full_root(value: &Value) {
    assert!(
        value.as_str().is_some_and(|root| {
            root.len() == 71
                && root.starts_with("sha256:")
                && root[7..]
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        }),
        "expected one full lowercase SHA-256 root, got {value}"
    );
}

fn clone_and_replay(source: &Path, destination: &Path, home: &Path) -> Value {
    let cloned = Command::new("git")
        .args(["clone", "-q"])
        .arg(source)
        .arg(destination)
        .output()
        .expect("clone repository");
    assert!(
        cloned.status.success(),
        "git clone: {}",
        String::from_utf8_lossy(&cloned.stderr)
    );
    success_json(&run(destination, None, home, &["replay", ".", "--json"]))
}

fn event_receipts(repository: &Path) -> Vec<Value> {
    let mut paths = std::fs::read_dir(repository.join(".vela/authority/events"))
        .expect("read authority events")
        .map(|entry| entry.expect("authority event entry").path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(&path).expect("authority event bytes");
            let value: Value = serde_json::from_slice(&bytes).expect("authority event JSON");
            json!({
                "id": path.file_stem().and_then(|stem| stem.to_str()).expect("event id"),
                "root": vela_protocol::canonical::sha256_root(&bytes),
                "kind": value["content"]["kind"],
            })
        })
        .collect()
}

fn authority_record_receipts(repository: &Path) -> Vec<Value> {
    let mut paths = std::fs::read_dir(repository.join(".vela/authority/records"))
        .expect("read authority records")
        .map(|entry| entry.expect("authority record entry").path())
        .collect::<Vec<_>>();
    paths.sort();
    let mut receipts = paths
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(&path).expect("authority record bytes");
            let envelope: Value = serde_json::from_slice(&bytes).expect("authority envelope JSON");
            let payload = vela_protocol::dsse::decode_base64(
                "authority payload",
                envelope["payload"].as_str().expect("authority payload"),
            )
            .expect("decode authority payload");
            let record: Value = serde_json::from_slice(&payload).expect("authority record JSON");
            json!({
                "id": path.file_name().and_then(|name| name.to_str()).and_then(|name| name.strip_suffix(".dsse.json")).expect("authority record id"),
                "root": vela_protocol::canonical::sha256_root(&payload),
                "sequence": record["content"]["sequence"],
                "principal_id": record["content"]["principal"]["principal_id"],
                "after_event_log_root": record["content"]["after_event_log_root"],
            })
        })
        .collect::<Vec<_>>();
    receipts.sort_by_key(|receipt| receipt["sequence"].as_u64().expect("record sequence"));
    receipts
}

fn reference_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/portable-divergence")
}

#[test]
fn frozen_distinct_principal_histories_match_every_bound_root() {
    let reference = reference_directory();
    let expected: Value = serde_json::from_slice(
        &std::fs::read(reference.join("expected.json")).expect("frozen expected roots"),
    )
    .expect("frozen expected JSON");
    assert_ne!(
        expected["accept"]["principal_id"], expected["reject"]["principal_id"],
        "the frozen histories must authenticate different local principals"
    );
    let temporary = tempfile::tempdir().expect("frozen replay directory");
    let home = temporary.path().join("home");
    std::fs::create_dir(&home).expect("frozen replay home");

    let mut retained_submissions = Vec::new();
    let mut retained_claims = Vec::new();
    for name in ["accept", "reject"] {
        let history = &expected[name];
        let bundle = reference.join(history["bundle"].as_str().expect("bundle path"));
        assert_eq!(
            vela_protocol::canonical::sha256_root(
                &std::fs::read(&bundle).expect("frozen Git bundle")
            ),
            history["bundle_root"]
        );
        let verified = Command::new("git")
            .args(["bundle", "verify"])
            .arg(&bundle)
            .output()
            .expect("verify Git bundle");
        assert!(
            verified.status.success(),
            "git bundle verify: {}",
            String::from_utf8_lossy(&verified.stderr)
        );
        let repository = temporary.path().join(format!("{name}-repository"));
        let cloned = Command::new("git")
            .args(["clone", "-q"])
            .arg(&bundle)
            .arg(&repository)
            .output()
            .expect("clone frozen Git bundle");
        assert!(
            cloned.status.success(),
            "git clone: {}",
            String::from_utf8_lossy(&cloned.stderr)
        );

        let replay = success_json(&run(&repository, None, &home, &["replay", ".", "--json"]));
        assert_eq!(replay["repository_id"], history["repository_id"]);
        assert_eq!(replay["origin_id"], history["origin_id"]);
        assert_eq!(replay["origin_root"], history["origin_root"]);
        assert_eq!(replay["authority_keyset_root"], history["keyset_root"]);
        assert_eq!(replay["authority_model_root"], history["model_root"]);
        assert_eq!(replay["repository_root"], history["repository_root"]);
        assert_eq!(replay["git_commit"], history["git_commit"]);
        assert_eq!(replay["git_tree"], history["git_tree"]);
        assert_eq!(
            replay["counts"]["accepted_claims"],
            history["accepted_claim_count"]
        );
        assert_eq!(
            replay["counts"]["pending_claims"],
            history["pending_claim_count"]
        );

        let records = authority_record_receipts(&repository);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["id"], history["sequence_one_record_id"]);
        assert_eq!(records[0]["root"], history["sequence_one_record_root"]);
        assert_eq!(records[0]["principal_id"], history["principal_id"]);
        assert_eq!(records[1]["id"], history["decision_record_id"]);
        assert_eq!(records[1]["root"], history["decision_record_root"]);
        assert_eq!(records[1]["principal_id"], history["principal_id"]);
        assert_eq!(
            records[1]["after_event_log_root"],
            history["event_log_root"]
        );
        assert_eq!(Value::Array(event_receipts(&repository)), history["events"]);

        let projection = success_json(&run(
            &repository,
            None,
            &home,
            &["projection", ".", "--json"],
        ));
        assert_eq!(projection["projection_root"], history["projection_root"]);
        assert_eq!(projection["claims"][0]["standing"], history["standing"]);
        assert_eq!(
            projection["claims"][0]["proposal_status"],
            history["proposal_status"]
        );

        let manifest = repository_manifest(&repository);
        let submission_path = manifest["submissions"][0]["path"]
            .as_str()
            .expect("frozen Submission path");
        retained_submissions.push(
            std::fs::read(repository.join(submission_path)).expect("frozen Submission bytes"),
        );
        let claim_path = format!("records/claims/sha256/{}.json", &CLAIM_ROOT[7..]);
        retained_claims
            .push(std::fs::read(repository.join(claim_path)).expect("frozen Claim bytes"));
    }
    assert_eq!(retained_submissions[0], retained_submissions[1]);
    assert_eq!(
        vela_protocol::canonical::sha256_root(&retained_submissions[0]),
        SUBMISSION_ROOT
    );
    assert_eq!(retained_claims[0], retained_claims[1]);
    assert_eq!(
        vela_protocol::canonical::sha256_root(&retained_claims[0]),
        CLAIM_ROOT
    );
}

#[test]
fn one_portable_submission_replays_under_divergent_local_decisions() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let accept_agent_root = temporary.path().join("accept-agent");
    let reject_agent_root = temporary.path().join("reject-agent");
    let accept_home = temporary.path().join("accept-home");
    let reject_home = temporary.path().join("reject-home");
    let verifier_home = temporary.path().join("verifier-home");
    for directory in [
        &accept_agent_root,
        &reject_agent_root,
        &accept_home,
        &reject_home,
        &verifier_home,
    ] {
        std::fs::create_dir_all(directory).expect("create fixture directory");
    }
    let accept_agent = EphemeralAgent::start(&accept_agent_root, "portable divergence accept");
    let reject_agent = EphemeralAgent::start(&reject_agent_root, "portable divergence reject");

    let accept_repository = temporary.path().join("accept-repository");
    let reject_repository = temporary.path().join("reject-repository");
    let accept_repository_text = accept_repository.to_string_lossy().into_owned();
    let reject_repository_text = reject_repository.to_string_lossy().into_owned();

    let accept_init_output = run_on_device(
        temporary.path(),
        Some(accept_agent.socket()),
        &accept_home,
        ACCEPT_DEVICE,
        &[
            "init",
            &accept_repository_text,
            "--name",
            "Portable divergence accept fixture",
            "--scope",
            "Govern one synthetic bounded Submission independently.",
            "--json",
        ],
    );
    let accept_anchor =
        RemoveAnchorOnDrop::from_init_json(&String::from_utf8_lossy(&accept_init_output.stdout))
            .expect("accept init trust anchor");
    let accept_init = success_json(&accept_init_output);

    let reject_init_output = run_on_device(
        temporary.path(),
        Some(reject_agent.socket()),
        &reject_home,
        REJECT_DEVICE,
        &[
            "init",
            &reject_repository_text,
            "--name",
            "Portable divergence reject fixture",
            "--scope",
            "Govern one synthetic bounded Submission independently.",
            "--json",
        ],
    );
    let reject_anchor =
        RemoveAnchorOnDrop::from_init_json(&String::from_utf8_lossy(&reject_init_output.stdout))
            .expect("reject init trust anchor");
    let reject_init = success_json(&reject_init_output);

    assert_ne!(accept_init["repository_id"], reject_init["repository_id"]);
    assert_ne!(
        accept_init["authority"]["principal_id"],
        reject_init["authority"]["principal_id"]
    );
    assert_ne!(
        accept_init["authority"]["key_id"],
        reject_init["authority"]["key_id"]
    );
    assert_ne!(
        accept_init["authority"]["record_root"],
        reject_init["authority"]["record_root"]
    );
    assert_ne!(
        accept_init["authority"]["keyset_root"],
        reject_init["authority"]["keyset_root"]
    );
    assert_ne!(
        accept_init["authority"]["policy_root"],
        reject_init["authority"]["policy_root"]
    );
    assert_ne!(accept_anchor.0, reject_anchor.0);
    configure_git_identity(&accept_repository);
    configure_git_identity(&reject_repository);

    let fixture_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/current-objects");
    let workspace_root = fixture_root
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let flow: Value = serde_json::from_slice(
        &std::fs::read(workspace_root.join("examples/portable-divergence/flow.json"))
            .expect("portable divergence flow"),
    )
    .expect("portable divergence flow JSON");
    assert_eq!(flow["fixture"]["submission_id"], SUBMISSION_ID);
    assert_eq!(flow["fixture"]["submission_root"], SUBMISSION_ROOT);
    assert_eq!(flow["fixture"]["producer"], PRODUCER);
    assert_eq!(flow["derived_claim"]["claim_id"], CLAIM_ID);
    assert_eq!(flow["derived_claim"]["claim_root"], CLAIM_ROOT);
    assert_eq!(
        flow["local_histories"]["accept"]["synthetic_device"],
        ACCEPT_DEVICE
    );
    assert_eq!(
        flow["local_histories"]["accept"]["performer"],
        ACCEPT_PERFORMER
    );
    assert_eq!(
        flow["local_histories"]["accept"]["session_ref"],
        ACCEPT_SESSION_REF
    );
    assert_eq!(
        flow["local_histories"]["reject"]["performer"],
        REJECT_PERFORMER
    );
    assert_eq!(
        flow["local_histories"]["reject"]["session_ref"],
        REJECT_SESSION_REF
    );
    assert_eq!(
        flow["local_histories"]["reject"]["synthetic_device"],
        REJECT_DEVICE
    );
    let submission_path = fixture_root.join("submission.json");
    let submission_path_text = submission_path.to_string_lossy().into_owned();

    let accepted_submission = success_json(&run(
        &accept_repository,
        None,
        &accept_home,
        &["submit", &submission_path_text, "--repo", ".", "--json"],
    ));
    let rejected_submission = success_json(&run(
        &reject_repository,
        None,
        &reject_home,
        &["submit", &submission_path_text, "--repo", ".", "--json"],
    ));

    for submitted in [&accepted_submission, &rejected_submission] {
        assert_eq!(submitted["submission_id"], SUBMISSION_ID);
        assert_eq!(submitted["submission_root"], SUBMISSION_ROOT);
        assert_eq!(submitted["route"], "pending_review");
        assert_eq!(submitted["accepted_state_changed"], false);
    }
    assert_eq!(
        accepted_submission["claim_id"], rejected_submission["claim_id"],
        "the same portable Claim request must derive the same Claim identity"
    );

    let accept_pending = repository_manifest(&accept_repository);
    let reject_pending = repository_manifest(&reject_repository);
    assert_eq!(accept_pending["submissions"], reject_pending["submissions"]);
    assert_eq!(
        accept_pending["pending_claims"],
        reject_pending["pending_claims"]
    );
    let claim_id = accept_pending["pending_claims"][0]["claim_id"]
        .as_str()
        .expect("pending Claim id")
        .to_string();
    let claim_root = accept_pending["pending_claims"][0]["claim_root"]
        .as_str()
        .expect("pending Claim root")
        .to_string();
    assert_eq!(claim_id, CLAIM_ID);
    assert_eq!(claim_root, CLAIM_ROOT);
    let retained_submission_path = accept_pending["submissions"][0]["path"]
        .as_str()
        .expect("retained Submission path");
    assert_eq!(
        std::fs::read(accept_repository.join(retained_submission_path))
            .expect("accepted repository Submission bytes"),
        std::fs::read(reject_repository.join(retained_submission_path))
            .expect("rejected repository Submission bytes")
    );
    assert_eq!(
        std::fs::read(accept_repository.join(retained_submission_path))
            .expect("retained Submission bytes"),
        std::fs::read(&submission_path).expect("source Submission bytes")
    );

    let method_path = "verification/portable-divergence.json";
    std::fs::create_dir_all(accept_repository.join("verification"))
        .expect("verification method directory");
    let method = ReviewMethodV1 {
        schema: vela_protocol::review_method::REVIEW_METHOD_V1_SCHEMA.into(),
        profile: "portable-divergence-recompute-v1".into(),
        property: REQUIREMENT.into(),
        question: "Do the exact retained fixture bytes equal the bounded result 42?".into(),
        reviewer: ReviewPerformerV1 {
            kind: "deterministic_tool".into(),
            display_name: "Portable divergence fixture checker".into(),
            identifier: "exact-bytes-42".into(),
            provider: None,
            version: None,
        },
        attested_by_actor_id: VERIFIER.into(),
        procedure: vec![
            "Read the retained Artifact and compare it byte-for-byte with `42\\n`.".into(),
        ],
        required_output: vec!["Report only whether the exact bounded comparison passes.".into()],
        does_not_establish: vec![
            "Scientific truth, acceptance outside this Repository, or global consensus.".into(),
        ],
    };
    std::fs::write(
        accept_repository.join(method_path),
        vela_protocol::canonical::to_canonical_bytes(&method).expect("canonical Review Method"),
    )
    .expect("write Review Method");
    git(&accept_repository, &["add", "--", method_path]);
    git(
        &accept_repository,
        &[
            "commit",
            "-qm",
            "Retain portable divergence verification method",
        ],
    );

    let accepted_proposal = accepted_submission["proposal_id"]
        .as_str()
        .expect("accept Proposal id");
    let verified = success_json(&run(
        &accept_repository,
        None,
        &verifier_home,
        &[
            "verification",
            "record",
            ".",
            accepted_proposal,
            "--profile",
            "portable-divergence-recompute-v1",
            "--method",
            method_path,
            "--property",
            REQUIREMENT,
            "--outcome",
            "pass",
            "--does-not-establish",
            "Scientific truth, acceptance outside this Repository, or global consensus.",
            "--independent-of",
            PRODUCER,
            "--as",
            VERIFIER,
            "--json",
        ],
    ));
    assert_eq!(verified["outcome"], "pass");
    assert_eq!(verified["accepted_event_delta"], 0);

    let accept_inbox = success_json(&run(
        &accept_repository,
        None,
        &accept_home,
        &["review", "inbox", ".", "--json"],
    ));
    let accept_entry_root = accept_inbox["entries"][0]["entry_root"]
        .as_str()
        .expect("accept Inbox entry root");
    let accepted = success_json(&run_on_device(
        &accept_repository,
        Some(accept_agent.socket()),
        &accept_home,
        ACCEPT_DEVICE,
        &[
            "review",
            "accept",
            ".",
            accepted_proposal,
            "--if-entry-root",
            accept_entry_root,
            "--reason",
            ACCEPT_REASON,
            "--as",
            ACCEPT_PERFORMER,
            "--session-ref",
            ACCEPT_SESSION_REF,
            "--json",
        ],
    ));
    assert_eq!(accepted["action"], "accept");
    assert_eq!(accepted["scientific_state_changed"], true);

    let rejected_proposal = rejected_submission["proposal_id"]
        .as_str()
        .expect("reject Proposal id");
    let reject_inbox = success_json(&run(
        &reject_repository,
        None,
        &reject_home,
        &["review", "inbox", ".", "--json"],
    ));
    let reject_entry_root = reject_inbox["entries"][0]["entry_root"]
        .as_str()
        .expect("reject Inbox entry root");
    let rejected = success_json(&run_on_device(
        &reject_repository,
        Some(reject_agent.socket()),
        &reject_home,
        REJECT_DEVICE,
        &[
            "review",
            "reject",
            ".",
            rejected_proposal,
            "--if-entry-root",
            reject_entry_root,
            "--reason",
            REJECT_REASON,
            "--as",
            REJECT_PERFORMER,
            "--session-ref",
            REJECT_SESSION_REF,
            "--json",
        ],
    ));
    assert_eq!(rejected["action"], "reject");
    assert_eq!(rejected["scientific_state_changed"], false);
    assert_eq!(
        accepted["authority_principal_id"],
        accept_init["authority"]["principal_id"]
    );
    assert_eq!(
        rejected["authority_principal_id"],
        reject_init["authority"]["principal_id"]
    );
    assert_ne!(
        accepted["authority_principal_id"],
        rejected["authority_principal_id"]
    );
    assert_ne!(
        accepted["authority_record_root"], rejected["authority_record_root"],
        "independent Repository Decisions must extend different authority histories"
    );
    assert_eq!(authority_record_count(&accept_repository), 2);
    assert_eq!(authority_record_count(&reject_repository), 2);

    let accept_replay = success_json(&run(
        &accept_repository,
        None,
        &accept_home,
        &["replay", ".", "--json"],
    ));
    let reject_replay = success_json(&run(
        &reject_repository,
        None,
        &reject_home,
        &["replay", ".", "--json"],
    ));
    assert_eq!(accept_replay["counts"]["accepted_claims"], 1);
    assert_eq!(accept_replay["counts"]["pending_claims"], 0);
    assert_eq!(reject_replay["counts"]["accepted_claims"], 0);
    assert_eq!(reject_replay["counts"]["pending_claims"], 0);
    assert_ne!(
        accept_replay["repository_root"],
        reject_replay["repository_root"]
    );
    assert_full_root(&accept_replay["repository_root"]);
    assert_full_root(&reject_replay["repository_root"]);
    assert_full_root(&accepted["authority_record_root"]);
    assert_full_root(&rejected["authority_record_root"]);

    let accept_clone = clone_and_replay(
        &accept_repository,
        &temporary.path().join("accept-clone"),
        &accept_home,
    );
    let reject_clone = clone_and_replay(
        &reject_repository,
        &temporary.path().join("reject-clone"),
        &reject_home,
    );
    assert_eq!(
        accept_clone["repository_root"],
        accept_replay["repository_root"]
    );
    assert_eq!(accept_clone["git_commit"], accept_replay["git_commit"]);
    assert_eq!(accept_clone["git_tree"], accept_replay["git_tree"]);
    assert_eq!(
        reject_clone["repository_root"],
        reject_replay["repository_root"]
    );
    assert_eq!(reject_clone["git_commit"], reject_replay["git_commit"]);
    assert_eq!(reject_clone["git_tree"], reject_replay["git_tree"]);

    let accepted_manifest = repository_manifest(&accept_repository);
    assert_eq!(
        accepted_manifest["accepted_claims"][0]["claim_id"],
        claim_id
    );
    assert_eq!(
        accepted_manifest["accepted_claims"][0]["claim_root"],
        claim_root
    );
    let rejected_review = success_json(&run(
        &reject_repository,
        None,
        &reject_home,
        &["review", "show", ".", rejected_proposal, "--json"],
    ));
    assert_eq!(rejected_review["status"], "rejected");
    assert_eq!(rejected_review["claim"]["claim_id"], claim_id);
}
