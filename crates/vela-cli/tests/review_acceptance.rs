//! End-to-end `vela review accept` through the product binary.
//!
//! Acceptance is the only write in the protocol that moves a Claim's Standing,
//! and the authority argument rests on it: an authorized attributed Decision,
//! signed by the repository authority over a contiguous record chain, is the sole
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
use std::process::Command;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde_json::Value;
use vela_protocol::authority::{AuthorityEnvelopeV1, AuthorityEventV1, AuthorityRecordV1};

mod support;
use support::{EphemeralAgent, RemoveAnchorOnDrop, run_with_isolated_home as run, success_json};

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

/// Test-only canonical commitment to the accepted Claim slice that constitutes
/// accepted Standing. The Repository root is intentionally broader: routine
/// Submission and Verification intake changes it while leaving this slice
/// unchanged.
fn accepted_standing_commitment(repository_path: &Path) -> String {
    let repository: Value = serde_json::from_slice(
        &std::fs::read(repository_path.join(".vela/repository.json"))
            .expect("repository manifest bytes"),
    )
    .expect("repository manifest JSON");
    format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&repository["accepted_claims"])
            .expect("accepted Standing commitment")
    )
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
    let requirements = [
        "Replay the retained artifact bytes.",
        "Inspect the retained caveat boundary.",
    ];
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
            requirements[0],
            "--requires-verification",
            requirements[1],
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
    let standing_before_evidence = accepted_standing_commitment(&repository_path);

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
            requirements[0],
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
    let second_verified = success_json(&run(
        &repository_path,
        None,
        &verifier_home,
        &[
            "verification",
            "record",
            ".",
            &proposal_id,
            "--profile",
            "caveat-boundary-v1",
            "--method",
            method_path,
            "--property",
            requirements[1],
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
    assert_eq!(second_verified["outcome"], "pass");
    assert_eq!(second_verified["accepted_event_delta"], 0);
    assert_ne!(
        second_verified["verification_record_id"], verified["verification_record_id"],
        "separate scoped checks must retain separate Verification Records"
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
    assert_eq!(evidenced["counts"]["verifications"], 2);
    assert_eq!(evidenced["counts"]["accepted_claims"], 0);
    assert_eq!(evidenced["counts"]["pending_claims"], 1);
    assert_eq!(
        accepted_standing_commitment(&repository_path),
        standing_before_evidence,
        "multiple passing Verification Records must not change accepted Standing"
    );
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
    /* `pending_review` is the Proposal's status, not the Claim's standing.
    Check both axes because the Verification pass moved neither. */
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
    assert_eq!(
        inbox["entries"][0]["verification_requirements"],
        serde_json::json!(requirements)
    );
    assert_eq!(
        inbox["entries"][0]["verification_records"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    let mut satisfied = inbox["entries"][0]["verification_records"]
        .as_array()
        .expect("Verification Record views")
        .iter()
        .flat_map(|record| {
            record["satisfies_requirements"]
                .as_array()
                .expect("satisfied requirements")
                .iter()
                .map(|value| value.as_str().expect("requirement").to_string())
        })
        .collect::<Vec<_>>();
    satisfied.sort();
    let mut expected_requirements = requirements.map(str::to_string).to_vec();
    expected_requirements.sort();
    assert_eq!(satisfied, expected_requirements);
    let entry_root = inbox["entries"][0]["entry_root"]
        .as_str()
        .expect("Decision Inbox entry root")
        .to_string();
    let head_before = git(&repository_path, &["rev-parse", "HEAD^{commit}"]);
    let chain_before_decision = contiguous_authority_chain(&repository_path);

    let reason = "Accept the exact bounded fixture Claim on its independent passing Verification.";
    let decision_actor = "agent:review-acceptance-fixture";
    let session_ref = "entire:checkpoint:review-acceptance-fixture";
    let refused = run(
        &repository_path,
        None,
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
            "--as",
            decision_actor,
            "--session-ref",
            session_ref,
            "--json",
        ],
    );
    assert!(!refused.status.success());
    let refused: Value =
        serde_json::from_slice(&refused.stdout).expect("authority-refusal error JSON");
    assert_eq!(refused["error"]["code"], "authority_refused");
    assert!(
        refused["error"]["hint"]
            .as_str()
            .is_some_and(|hint| hint.contains("ssh-add -l") && hint.contains("--as"))
    );
    assert_eq!(
        git(&repository_path, &["rev-parse", "HEAD^{commit}"]),
        head_before,
        "authority refusal must not publish a Decision"
    );
    assert_eq!(
        contiguous_authority_chain(&repository_path),
        chain_before_decision,
        "authority refusal must not extend the chain"
    );

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
            "--as",
            decision_actor,
            "--session-ref",
            session_ref,
            "--json",
        ],
    ));
    assert_eq!(accepted["schema"], "vela.review-decision.v5");
    /* `.v3` carried the reviewed path under `"repository"`, beside a
    `repository_id` naming the thing at that path — one document with two
    vocabularies, in the only place where the retired noun had a consumer.
    `.v5` preserves `repository_path` and adds the attributed performer.
    Both halves are asserted so a revert fails here. */
    assert!(
        accepted["repository_path"].is_string(),
        "`repository_path` is the published path key of .v5:\n{accepted}"
    );
    assert!(
        accepted.get("frontier").is_none(),
        "`.v5` retains no retired route noun:\n{accepted}"
    );
    assert_eq!(accepted["command"], "review.accept");
    assert_eq!(accepted["action"], "accept");
    assert_eq!(accepted["scientific_state_changed"], true);
    assert_eq!(
        accepted["verification_set_root"], inbox["entries"][0]["inputs"]["verification_set_root"],
        "the authorized Decision must consume the exact rooted multi-Verification set shown to the reviewer"
    );
    assert_eq!(accepted["claim_id"], claim_id.as_str());
    assert_eq!(accepted["reason"], reason);
    assert_eq!(accepted["actor_id"], decision_actor);
    assert_eq!(accepted["actor_class"], "agent");
    assert_eq!(accepted["session_ref"], session_ref);
    assert_eq!(accepted["transaction_signer"], "repository_authority");
    assert_eq!(accepted["performer_key_read"], false);
    assert_eq!(accepted["performer"]["actor_id"], decision_actor);
    assert_eq!(accepted["performer"]["actor_class"], "agent");
    assert_eq!(accepted["performer"]["session_ref"], session_ref);
    assert_eq!(accepted["performer"]["key_read"], false);
    assert_eq!(
        accepted["authority"]["principal_id"],
        accepted["authority_principal_id"]
    );
    assert_eq!(
        accepted["authority"]["authentication"],
        accepted["authentication"]
    );
    assert_eq!(
        accepted["authority"]["transaction_signer"],
        "repository_authority"
    );
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
        decided["counts"]["verifications"], 2,
        "acceptance must not manufacture further evidence"
    );
    assert_ne!(
        accepted_standing_commitment(&repository_path),
        standing_before_evidence,
        "only the admitted acceptance changes the accepted Standing commitment"
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
    assert_eq!(asserted["content"]["actor"]["type"], "agent");
    assert_eq!(asserted["content"]["actor"]["id"], decision_actor);
    assert_eq!(
        asserted["content"]["payload"]["decision_performer"]["session_ref"],
        session_ref
    );
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

    let reviewed = success_json(&run(
        &repository_path,
        None,
        &producer_home,
        &["review", "show", ".", &proposal_id, "--json"],
    ));
    assert_eq!(reviewed["decision"]["actor"], decision_actor);
    assert_eq!(reviewed["decision"]["actor_class"], "agent");
    assert_eq!(reviewed["decision"]["session_ref"], session_ref);
    assert_eq!(
        reviewed["decision"]["authority_principal_id"],
        accepted["authority_principal_id"]
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

    // One Core-owned projection now carries the same current scientific state
    // without asking a consumer to decode the Repository index, DSSE, or
    // authority Events. It is deterministic and independent of checkout path.
    let status_before_projection = Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(&repository_path)
        .output()
        .expect("inspect repository before projection");
    assert!(status_before_projection.status.success());
    let projection = success_json(&run(
        &repository_path,
        None,
        &producer_home,
        &["projection", ".", "--json"],
    ));
    assert_eq!(projection["schema"], "vela.repository-projection.v1");
    assert_eq!(projection["authority_effect"], "none");
    assert_eq!(
        projection["repository"]["repository_root"],
        decided["repository_root"]
    );
    assert_eq!(projection["counts"]["claims"], 1);
    assert_eq!(projection["counts"]["accepted_claims"], 1);
    assert_eq!(projection["claims"][0]["claim_id"], claim_id);
    assert_eq!(projection["claims"][0]["standing"], "accepted");
    assert_eq!(projection["claims"][0]["proposal_status"], "accepted");
    let mut projection_commitment = projection.clone();
    projection_commitment
        .as_object_mut()
        .expect("projection object")
        .remove("projection_root");
    assert_eq!(
        projection["projection_root"],
        format!(
            "sha256:{}",
            vela_protocol::canonical::sha256_canonical(&projection_commitment)
                .expect("projection commitment")
        )
    );
    let projection_path = temporary.path().join("repository-projection.json");
    std::fs::write(
        &projection_path,
        serde_json::to_vec(&projection).expect("serialize projection"),
    )
    .expect("write projection for schema validation");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let schema_check = Command::new("uv")
        .current_dir(workspace)
        .args([
            "run",
            "--project",
            "conformance",
            "--locked",
            "python",
            "-c",
            "import json,sys; from jsonschema import Draft202012Validator; schema=json.load(open(sys.argv[1])); value=json.load(open(sys.argv[2])); errors=list(Draft202012Validator(schema).iter_errors(value)); assert not errors, '\\n'.join(str(error) for error in errors)",
        ])
        .arg(workspace.join("schemas/repository-projection.schema.json"))
        .arg(&projection_path)
        .output()
        .expect("run projection schema validation");
    assert!(
        schema_check.status.success(),
        "Core projection must satisfy its published schema: {}{}",
        String::from_utf8_lossy(&schema_check.stdout),
        String::from_utf8_lossy(&schema_check.stderr)
    );
    assert_eq!(
        projection["proposals"][0]["decision"]["decision_event"]["semantic_event_id"],
        review["semantic_event_id"]
    );
    assert_eq!(
        projection["proposals"][0]["decision"]["applied_event"]["semantic_event_id"],
        asserted["semantic_event_id"]
    );
    assert_ne!(
        projection["proposals"][0]["decision"]["decision_event"]["authority_event_id"],
        projection["proposals"][0]["decision"]["applied_event"]["authority_event_id"],
        "the review Decision Event and applied scientific Event are distinct"
    );
    assert_eq!(
        projection,
        success_json(&run(
            &repository_path,
            None,
            &producer_home,
            &["projection", ".", "--json"],
        )),
        "repeated projection must be byte-semantically deterministic"
    );
    assert_eq!(
        projection,
        success_json(&run(
            &clone,
            None,
            &producer_home,
            &["projection", ".", "--json"],
        )),
        "the same commit in another checkout path must project identically"
    );
    let checked_out_history = Command::new("git")
        .args(["checkout", "-q", &head_before])
        .current_dir(&clone)
        .output()
        .expect("checkout the exact pre-Decision commit");
    assert!(
        checked_out_history.status.success(),
        "historical checkout: {}",
        String::from_utf8_lossy(&checked_out_history.stderr)
    );
    let historical_projection = success_json(&run(
        &clone,
        None,
        &producer_home,
        &["projection", ".", "--json"],
    ));
    assert_eq!(
        historical_projection["repository"]["repository_root"],
        evidenced["repository_root"]
    );
    assert_eq!(historical_projection["counts"]["accepted_claims"], 0);
    assert_eq!(historical_projection["counts"]["pending_claims"], 1);
    assert_eq!(historical_projection["counts"]["pending_review"], 1);
    assert_eq!(historical_projection["claims"][0]["standing"], "unassessed");
    assert_eq!(
        historical_projection["handoff"]["active_pending_claim_ids"][0],
        claim_id
    );
    assert_eq!(
        historical_projection["handoff"]["inactive_unassessed_claim_ids"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        historical_projection["claims"][0]["proposal_status"],
        "pending_review"
    );
    assert_eq!(
        historical_projection["decision_inbox_summary"]["pending_count"],
        1
    );
    let status_after_projection = Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(&repository_path)
        .output()
        .expect("inspect repository after projection");
    assert!(status_after_projection.status.success());
    assert_eq!(
        status_before_projection.stdout, status_after_projection.stdout,
        "projection is a read-only export"
    );
    let projection_bytes = serde_json::to_vec(&projection).expect("serialize projection");
    assert!(
        !projection_bytes
            .windows(repository_path.as_os_str().len())
            .any(|window| window == repository_path.as_os_str().as_encoded_bytes()),
        "projection must not embed its checkout path"
    );

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
    assert!(
        why_after["chain"].get("standing_basis").is_none(),
        "the one current authority chain needs no compaction-era basis label"
    );
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
