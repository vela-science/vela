//! End-to-end `vela review accept` through the product binary.
//!
//! Acceptance is the only write in the protocol that moves a Claim's Standing,
//! and the authority argument rests on it: an authorized human Decision, signed
//! by the repository authority over a contiguous record chain, is the sole
//! thing that admits the scientific Event. Nothing else in the suite executes a
//! successful accept, so nothing else holds that argument to its claims.
//!
//! The same fixture carries the negative the protocol insists on. Immediately
//! before the Decision the Proposal already has an independent passing
//! Verification Record. The Claim is still `unassessed` over a Proposal still
//! `pending_review`, no authority Event exists, and the authority chain has not
//! moved. A pass is evidence, not acceptance.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde_json::Value;
use vela_protocol::authority::{AuthorityEnvelopeV1, AuthorityEventV1, AuthorityRecordV1};

mod support;
use support::{EphemeralAgent, RemoveAnchorOnDrop};

/// Run the product binary. `home` sandboxes the producer and verifier agent
/// keys `submit` and `verification record` mint on first use; the authority
/// trust anchor deliberately ignores `HOME` and is handled by
/// [`RemoveAnchorOnDrop`] instead.
fn run(cwd: &Path, socket: Option<&Path>, home: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(cwd)
        .args(args)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env_remove("VELA_AGENT_KEY_HEX");
    if let Some(socket) = socket {
        command.env("SSH_AUTH_SOCK", socket);
    } else {
        command.env("SSH_AUTH_SOCK", cwd.join("missing-ssh-agent.sock"));
    }
    command.output().expect("run vela")
}

fn success_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("decode Vela JSON")
}

fn git(repository_path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repository_path)
        .args(["-c", "user.name=Vela Test"])
        .args(["-c", "user.email=vela@example.invalid"])
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("UTF-8 git output")
        .trim()
        .to_string()
}

/// Exact bytes of one authority directory, keyed by filename.
///
/// Compared across a routine write this proves no authority file was added,
/// removed, or rewritten. Compared across the Decision it names precisely which
/// files the Decision appended.
fn authority_bytes(repository_path: &Path, directory: &str) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(repository_path.join(".vela/authority").join(directory))
        .expect("read authority directory")
        .map(|entry| {
            let path = entry.expect("authority directory entry").path();
            (
                path.file_name()
                    .and_then(|name| name.to_str())
                    .expect("UTF-8 filename")
                    .to_string(),
                std::fs::read(&path).expect("authority bytes"),
            )
        })
        .collect()
}

/// Every retained authority record, ordered by sequence, with the contiguity
/// the chain asserts about itself checked on the way through: sequences run
/// 1..=n with no gap, each record's `before_event_log_root` is the previous
/// record's `after_event_log_root`, and each record names the exact content
/// root of its predecessor.
fn contiguous_authority_chain(repository_path: &Path) -> Vec<AuthorityRecordV1> {
    let mut records = std::fs::read_dir(repository_path.join(".vela/authority/records"))
        .expect("read authority records")
        .map(|entry| {
            let bytes = std::fs::read(entry.expect("record entry").path()).expect("record bytes");
            let envelope: AuthorityEnvelopeV1 =
                serde_json::from_slice(&bytes).expect("authority DSSE envelope");
            let payload = BASE64_STANDARD
                .decode(envelope.payload)
                .expect("base64 authority payload");
            serde_json::from_slice::<AuthorityRecordV1>(&payload).expect("authority record")
        })
        .collect::<Vec<_>>();
    records.sort_by_key(|record| record.content.sequence);

    for (index, record) in records.iter().enumerate() {
        let expected_sequence = u64::try_from(index).expect("chain length fits u64") + 1;
        assert_eq!(
            record.content.sequence, expected_sequence,
            "authority chain skips sequence {expected_sequence}"
        );
        let Some(previous) = index.checked_sub(1).and_then(|before| records.get(before)) else {
            assert_eq!(
                record.content.previous_authority_record_root, None,
                "the sequence-one authority record must not name a predecessor"
            );
            continue;
        };
        assert_eq!(
            record.content.before_event_log_root, previous.content.after_event_log_root,
            "authority chain breaks at sequence {}",
            record.content.sequence
        );
        assert_eq!(
            record.content.previous_authority_record_root.as_deref(),
            Some(previous.root().expect("predecessor root").as_str()),
            "authority record {} does not name its predecessor's exact root",
            record.content.sequence
        );
    }
    records
}

fn authority_events(repository_path: &Path, ids: &[&str]) -> Vec<Value> {
    ids.iter()
        .map(|id| {
            let bytes = std::fs::read(
                repository_path
                    .join(".vela/authority/events")
                    .join(format!("{id}.json")),
            )
            .unwrap_or_else(|error| panic!("read admitted authority event {id}: {error}"));
            // Parsed twice on purpose: the typed form recovers the semantic
            // event identity the reducer consumes, the value form reads the
            // retained bytes without the test restating the schema.
            let typed: AuthorityEventV1 =
                serde_json::from_slice(&bytes).expect("retained authority event");
            let mut view: Value = serde_json::from_slice(&bytes).expect("authority event JSON");
            view["semantic_event_id"] =
                Value::from(typed.semantic_event_id().expect("semantic event id"));
            view
        })
        .collect()
}

#[test]
fn review_accept_admits_the_event_that_moves_standing() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let unique = temporary
        .path()
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("fixture")
        .to_string();
    let agent = EphemeralAgent::start(temporary.path(), "vela review accept test");
    let producer_home = temporary.path().join("producer-home");
    let verifier_home = temporary.path().join("verifier-home");
    for home in [&producer_home, &verifier_home] {
        std::fs::create_dir_all(home).expect("isolated agent home");
    }
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();

    let init = run(
        temporary.path(),
        Some(agent.socket()),
        &producer_home,
        &[
            "init",
            &repository_path_text,
            "--name",
            &format!("Review acceptance fixture {unique}"),
            "--scope",
            "Execute one authorized acceptance and hold its consequences.",
            "--json",
        ],
    );
    // Armed from init's own JSON, before anything can fail, so the anchor init
    // wrote into the OS account's trust store never outlives this test.
    let anchor = RemoveAnchorOnDrop::from_init_json(&String::from_utf8_lossy(&init.stdout))
        .expect("init reports the local trust anchor it installed");
    let initialized = success_json(&init);
    let record_root = initialized["authority"]["record_root"]
        .as_str()
        .expect("authority record root")
        .to_string();
    // Vela publishes through Git itself, so its own commits need an identity
    // this sandboxed HOME cannot supply globally. Without this the writes below
    // report `publication.state = "uncommitted"` and the Decision never gets a
    // committed preimage to plan against.
    git(&repository_path, &["config", "user.name", "Vela Test"]);
    git(
        &repository_path,
        &["config", "user.email", "vela@example.invalid"],
    );

    // The Decision needs an independently supplied sequence-one pin. Init has
    // already installed one, so pinning the root the operator carries out of
    // band must agree with it rather than write a second anchor.
    let pinned = success_json(&run(
        &repository_path,
        None,
        &producer_home,
        &[
            "authority",
            "trust",
            "pin",
            ".",
            "--record-root",
            &record_root,
            "--json",
        ],
    ));
    assert_eq!(pinned["operation"], "unchanged");
    assert!(pinned["writes"].as_array().expect("pin writes").is_empty());
    assert_eq!(
        pinned["authority_trust_anchor_path"].as_str(),
        Some(anchor.0.to_string_lossy().as_ref()),
        "the pinned anchor must be the exact file the test guard removes"
    );

    // One bounded Submission, routed to review and changing nothing.
    std::fs::write(
        repository_path.join("evidence.json"),
        b"{\"bounded\":true}\n",
    )
    .expect("write fixture artifact");
    let producer = "agent:review-accept-regression";
    let requirement = "Replay the retained artifact bytes.";
    let submitted = success_json(&run(
        &repository_path,
        None,
        &producer_home,
        &[
            "submit",
            "--repo",
            ".",
            "--claim",
            "The fixture artifact contains bounded JSON evidence.",
            "--type",
            "computational",
            "--replayability",
            "exact",
            "--artifact",
            "evidence.json:witness",
            "--caveat",
            "This fixture makes no unrestricted scientific claim.",
            "--requires-verification",
            requirement,
            "--as",
            producer,
            "--json",
        ],
    ));
    assert_eq!(submitted["route"], "pending_review");
    assert_eq!(submitted["accepted_event_delta"], 0);
    assert_eq!(
        submitted["publication"]["state"], "committed_local",
        "unexpected Submission publication: {submitted}"
    );
    let proposal_id = submitted["proposal_id"]
        .as_str()
        .expect("Proposal ID")
        .to_string();
    let claim_id = submitted["claim_id"]
        .as_str()
        .expect("Claim ID")
        .to_string();

    let method_path = "verification/exact-replay-v1.json";
    std::fs::create_dir_all(repository_path.join("verification")).expect("method directory");
    std::fs::write(
        repository_path.join(method_path),
        br#"{"command":"sha256sum evidence.json","schema":"vela.test-method.v1"}"#,
    )
    .expect("write verification method manifest");
    git(&repository_path, &["add", "--", method_path]);
    git(
        &repository_path,
        &["commit", "-qm", "Retain verification method"],
    );

    let events_before_evidence = authority_bytes(&repository_path, "events");
    let records_before_evidence = authority_bytes(&repository_path, "records");
    let chain_before_evidence = contiguous_authority_chain(&repository_path);

    let verifier = format!("verifier:review-accept-{unique}");
    let verified = success_json(&run(
        &repository_path,
        None,
        &verifier_home,
        &[
            "verification",
            "record",
            ".",
            &proposal_id,
            "--profile",
            "exact-replay-v1",
            "--method",
            method_path,
            "--property",
            requirement,
            "--outcome",
            "pass",
            "--does-not-establish",
            "Scientific acceptance.",
            "--independent-of",
            producer,
            "--as",
            &verifier,
            "--json",
        ],
    ));
    assert_eq!(verified["outcome"], "pass");
    assert_eq!(verified["accepted_event_delta"], 0);
    assert_eq!(
        verified["publication"]["state"], "committed_local",
        "unexpected Verification publication: {verified}"
    );

    // The negative the protocol insists on. The Proposal now carries an
    // independent passing Verification Record covering its only stated
    // requirement, and that alone changes nothing: no Standing transition, no
    // authority Event, no movement of the authority chain.
    let evidenced = success_json(&run(
        &repository_path,
        None,
        &producer_home,
        &["replay", ".", "--json"],
    ));
    assert_eq!(evidenced["counts"]["verifications"], 1);
    assert_eq!(evidenced["counts"]["accepted_claims"], 0);
    assert_eq!(evidenced["counts"]["pending_claims"], 1);
    assert_eq!(
        authority_bytes(&repository_path, "events"),
        events_before_evidence,
        "a passing Verification Record must not admit an authority Event"
    );
    assert_eq!(
        authority_bytes(&repository_path, "records"),
        records_before_evidence,
        "a passing Verification Record must not append an Authority Record"
    );
    assert_eq!(
        contiguous_authority_chain(&repository_path),
        chain_before_evidence,
        "a passing Verification Record must not move the authority chain"
    );
    let why_before = success_json(&run(
        &repository_path,
        None,
        &producer_home,
        &["why", ".", &claim_id, "--json"],
    ));
    /* Asserting `pending_review` here asserted the collapse: that is the
    Proposal's status, and reading it back as the Claim's standing is what
    `docs/TERMINOLOGY.md` now forbids. Both axes are checked, because the pass
    moved neither. */
    assert_eq!(
        why_before["standing"], "unassessed",
        "a passing Verification Record is not an acceptance"
    );
    assert_eq!(
        why_before["proposal_status"], "pending_review",
        "a passing Verification Record leaves the Proposal awaiting a Decision"
    );
    assert_eq!(
        why_before["interpretation"]["verification_is_acceptance"],
        false
    );
    assert_eq!(
        why_before["chain"]["authority_events"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );

    // Review the exact rooted entry, then decide it.
    let inbox = success_json(&run(
        &repository_path,
        None,
        &producer_home,
        &["review", "inbox", ".", "--json"],
    ));
    assert_eq!(inbox["entries"].as_array().map(Vec::len), Some(1));
    let entry_root = inbox["entries"][0]["entry_root"]
        .as_str()
        .expect("Decision Inbox entry root")
        .to_string();
    let head_before = git(&repository_path, &["rev-parse", "HEAD^{commit}"]);
    let chain_before_decision = contiguous_authority_chain(&repository_path);

    let reason = "Accept the exact bounded fixture Claim on its independent passing Verification.";
    let accepted = success_json(&run(
        &repository_path,
        Some(agent.socket()),
        &producer_home,
        &[
            "review",
            "accept",
            ".",
            &proposal_id,
            "--if-entry-root",
            &entry_root,
            "--reason",
            reason,
            "--json",
        ],
    ));
    assert_eq!(accepted["schema"], "vela.review-decision.v4");
    /* `.v3` carried the reviewed path under `"repository"`, beside a
    `repository_id` naming the thing at that path — one document with two
    vocabularies, in the only place where the retired noun had a consumer.
    `.v4` publishes it as `repository_path`, the key `replay` already uses.
    Both halves are asserted so a revert fails here. */
    assert!(
        accepted["repository_path"].is_string(),
        "`repository_path` is the published path key of .v4:\n{accepted}"
    );
    assert!(
        accepted.get("frontier").is_none(),
        "`.v4` retired the `repository_path` key:\n{accepted}"
    );
    assert_eq!(accepted["command"], "review.accept");
    assert_eq!(accepted["action"], "accept");
    assert_eq!(accepted["scientific_state_changed"], true);
    assert_eq!(accepted["claim_id"], claim_id.as_str());
    assert_eq!(accepted["reason"], reason);
    assert_eq!(accepted["transaction_signer"], "repository_authority");
    assert_eq!(accepted["human_key_read"], false);
    assert_eq!(
        accepted["repository_before"], evidenced["repository_root"],
        "the Decision must be planned against the exact reviewed repository"
    );

    // Standing changed.
    let decided = success_json(&run(
        &repository_path,
        None,
        &producer_home,
        &["replay", ".", "--json"],
    ));
    assert_eq!(decided["ok"], true);
    assert_eq!(decided["counts"]["accepted_claims"], 1);
    assert_eq!(decided["counts"]["pending_claims"], 0);
    assert_eq!(
        decided["counts"]["verifications"], 1,
        "acceptance must not manufacture further evidence"
    );
    assert_ne!(
        git(&repository_path, &["rev-parse", "HEAD^{commit}"]),
        head_before
    );

    // Events were admitted: exactly the two the Decision reports, appended and
    // nothing rewritten.
    let event_ids = accepted["event_ids"]
        .as_array()
        .expect("Decision event IDs")
        .iter()
        .map(|id| id.as_str().expect("event ID").to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        event_ids.len(),
        2,
        "an accept admits a domain and a review Event"
    );
    let events_after = authority_bytes(&repository_path, "events");
    for (name, bytes) in &events_before_evidence {
        assert_eq!(
            events_after.get(name),
            Some(bytes),
            "the Decision must append authority Events, never rewrite {name}"
        );
    }
    assert_eq!(
        events_after.len(),
        events_before_evidence.len() + 2,
        "the Decision must admit exactly the Events it reports"
    );
    let admitted = authority_events(
        &repository_path,
        &event_ids.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let asserted = admitted
        .iter()
        .find(|event| event["content"]["kind"] == "claim.asserted")
        .expect("the accept admits the claim.asserted domain Event");
    assert_eq!(asserted["content"]["target"]["id"], claim_id.as_str());
    assert_eq!(asserted["content"]["after_hash"], accepted["claim_root"]);
    assert_eq!(asserted["content"]["reason"], reason);
    let review = admitted
        .iter()
        .find(|event| event["content"]["kind"] == "review.accepted")
        .expect("the accept admits the review.accepted Event");
    assert_eq!(review["content"]["target"]["id"], proposal_id.as_str());
    assert_eq!(review["content"]["payload"]["verdict"], "accepted");
    assert_eq!(
        review["content"]["payload"]["applied_event_id"], asserted["semantic_event_id"],
        "the review Event must link the exact domain Event it applied"
    );
    assert_eq!(
        review["content"]["payload"]["repository_before"],
        evidenced["repository_root"]
    );
    assert_eq!(
        review["content"]["payload"]["repository_after"],
        decided["repository_root"]
    );

    // The authority chain stayed contiguous across the Decision.
    let chain_after = contiguous_authority_chain(&repository_path);
    assert_eq!(chain_after.len(), chain_before_decision.len() + 1);
    assert_eq!(
        &chain_after[..chain_before_decision.len()],
        chain_before_decision.as_slice(),
        "the Decision must extend the authority chain, never rewrite it"
    );
    let tail = chain_after.last().expect("decided authority record");
    let previous = &chain_before_decision[chain_before_decision.len() - 1];
    assert_eq!(
        tail.content.before_event_log_root,
        previous.content.after_event_log_root
    );
    assert_eq!(
        accepted["before_event_log_root"].as_str(),
        Some(tail.content.before_event_log_root.as_str())
    );
    assert_eq!(
        accepted["after_event_log_root"].as_str(),
        Some(tail.content.after_event_log_root.as_str())
    );
    assert_eq!(
        accepted["authority_record_id"].as_str(),
        Some(tail.record_id.as_str())
    );
    assert_eq!(
        accepted["authority_record_root"].as_str(),
        Some(tail.root().expect("decided record root").as_str())
    );
    assert_eq!(
        tail.content.event_ids, event_ids,
        "the covering record must name exactly the admitted Events"
    );

    // Replay still verifies, in place and from a clean clone that has only the
    // published bytes.
    let clone = temporary.path().join("decided-clone");
    let cloned = Command::new("git")
        .args(["clone", "-q"])
        .arg(&repository_path)
        .arg(&clone)
        .output()
        .expect("clone the decided repository");
    assert!(
        cloned.status.success(),
        "git clone: {}",
        String::from_utf8_lossy(&cloned.stderr)
    );
    let replayed = success_json(&run(
        &clone,
        None,
        &producer_home,
        &["replay", ".", "--json"],
    ));
    assert_eq!(replayed["ok"], true);
    assert_eq!(replayed["repository_root"], decided["repository_root"]);
    assert_eq!(replayed["counts"]["accepted_claims"], 1);

    // `vela why` reports the new Standing and the Decision's reason.
    let why_after = success_json(&run(
        &repository_path,
        None,
        &producer_home,
        &["why", ".", &claim_id, "--json"],
    ));
    assert_eq!(why_after["standing"], "accepted");
    assert_eq!(why_after["proposal_status"], "accepted");
    assert_eq!(why_after["claim_root"], accepted["claim_root"]);
    assert_eq!(why_after["chain"]["standing_basis"], "current_authority");
    let explained = why_after["chain"]["authority_events"]
        .as_array()
        .expect("explained authority events");
    assert_eq!(explained.len(), 2);
    for view in explained {
        assert_eq!(
            view["event"]["content"]["reason"], reason,
            "why must report the Decision's own reason"
        );
        assert!(
            event_ids.contains(
                &view["authority_event_id"]
                    .as_str()
                    .expect("authority event id")
                    .to_string()
            ),
            "why must explain the Standing with the Events the Decision admitted"
        );
    }
    assert_eq!(
        why_after["interpretation"]["verification_is_acceptance"],
        false
    );
    assert_eq!(why_after["interpretation"]["standing_is_derived"], true);

    let human_why = run(
        &repository_path,
        None,
        &producer_home,
        &["why", ".", &claim_id],
    );
    assert!(human_why.status.success());
    let human_why = String::from_utf8(human_why.stdout).expect("why text");
    assert!(human_why.contains("accepted"));
    assert!(human_why.contains(reason));
}
