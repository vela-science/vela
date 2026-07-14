//! Cross-surface regressions for ADR 0003's task-first trust boundary.

use std::path::Path;
use std::process::{Command, Output};

fn vela_bin() -> &'static str {
    env!("CARGO_BIN_EXE_vela")
}

fn run(dir: &Path, args: &[&str]) -> Output {
    run_with_env(dir, args, &[])
}

fn run_with_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(vela_bin());
    command
        .current_dir(dir)
        .args(args)
        .env("HOME", dir)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0");
    for (key, _) in std::env::vars() {
        if key.starts_with("VELA_") && key != "VELA_ADVICE" {
            command.env_remove(key);
        }
    }
    command.envs(env.iter().copied());
    command.output().expect("run vela")
}

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("HOME", dir)
        .output()
        .expect("run git")
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}: status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let output = git(dir, args);
    assert_success(&output, &format!("git {}", args.join(" ")));
    String::from_utf8(output.stdout).unwrap()
}

fn init_git_frontier(dir: &Path) {
    assert_success(
        &run(dir, &["init", ".", "--name", "task-first", "--json"]),
        "init frontier",
    );
    assert_success(
        &run(dir, &["id", "create", "--handle", "t"]),
        "create test identity",
    );
    assert_success(
        &git(dir, &["config", "user.email", "test@vela.invalid"]),
        "git email",
    );
    assert_success(&git(dir, &["config", "user.name", "Vela Test"]), "git name");
    assert_success(&git(dir, &["add", "-A"]), "stage baseline");
    assert_success(&git(dir, &["commit", "-qm", "baseline"]), "commit baseline");
}

fn write_receipt(dir: &Path, filename: &str, claim: &str) {
    write_receipt_with_artifact(dir, filename, claim, "w.json", br#"{"witness":true}"#);
}

fn write_receipt_with_artifact(
    dir: &Path,
    filename: &str,
    claim: &str,
    artifact_name: &str,
    artifact: &[u8],
) {
    write_receipt_with_artifact_as(
        dir,
        filename,
        claim,
        artifact_name,
        artifact,
        "agent:t",
        0x42,
    );
}

#[allow(clippy::too_many_arguments)]
fn write_receipt_with_artifact_as(
    dir: &Path,
    filename: &str,
    claim: &str,
    artifact_name: &str,
    artifact: &[u8],
    actor: &str,
    key_seed: u8,
) {
    use ed25519_dalek::SigningKey;
    use sha2::{Digest, Sha256};
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::receipt_v1::{
        ArtifactInput, ProducerReportedRun, ReceiptBuilder, ReceiptInput,
    };

    std::fs::create_dir_all(dir.join("artifacts")).unwrap();
    let artifact_path = format!("artifacts/{artifact_name}");
    std::fs::write(dir.join(&artifact_path), artifact).unwrap();
    let digest = hex::encode(Sha256::digest(artifact));
    let project = vela_protocol::repo::load_from_path(dir).unwrap();
    let event_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    );
    let operation_id = format!(
        "vop_{}",
        hex::encode(Sha256::digest(
            format!("{actor}\0{filename}\0{claim}\0{artifact_path}\0{digest}").as_bytes()
        ))
    );
    let at = "2026-07-13T12:00:00Z";
    let key = SigningKey::from_bytes(&[key_seed; 32]);
    let identity = IdentityBinding::build(
        IdentityBindingDraft {
            actor_id: actor.to_string(),
            actor_class: ActorClass::Agent,
            created_at: at.to_string(),
        },
        &key,
    )
    .unwrap();
    let input = ReceiptInput::new(
        claim.to_string(),
        "computational".to_string(),
        "exact".to_string(),
        vec![ArtifactInput::new(artifact_path, "witness".to_string(), Some(digest), None).unwrap()],
        vec!["fixture evidence only".to_string()],
        vec![
            ProducerReportedRun::producer_reported("fixture".to_string(), "pass".to_string())
                .unwrap(),
        ],
        actor.to_string(),
        at.to_string(),
        event_root,
        ".".to_string(),
        operation_id,
        "urn:vela:policy:none".to_string(),
    )
    .unwrap();
    let receipt = ReceiptBuilder::build(input, &identity).unwrap();
    std::fs::write(dir.join(filename), receipt.canonical_bytes().unwrap()).unwrap();
}

fn write_active_deny_policy(dir: &Path) {
    use ed25519_dalek::{Signer, SigningKey};
    use vela_protocol::acceptance_policy::{
        AcceptancePolicy, Outcome, PolicySignatureRecord, Quorum,
    };

    let frontier = vela_protocol::repo::load_from_path(dir).unwrap();
    let mut policy = AcceptancePolicy {
        schema: "vela.acceptance_policy.v0.1".to_string(),
        id: String::new(),
        frontier_id: frontier.frontier_id().to_string(),
        epoch: 1,
        issued_by: vec!["reviewer:deny-fixture".to_string()],
        quorum: Quorum {
            threshold: 1,
            eligible_roles: vec!["reviewer".to_string()],
        },
        rules: Vec::new(),
        default: Outcome::Deny,
        expires_at: "2099-12-31T23:59:59Z".to_string(),
        revocation_ref: None,
    };
    policy.id = policy.content_address();
    let key = SigningKey::from_bytes(&[0x24; 32]);
    let signed_at = "2026-07-12T00:00:00Z";
    let signature = key.sign(
        &vela_protocol::acceptance_policy::policy_signature_preimage(&policy, signed_at).unwrap(),
    );
    let policy_dir = dir.join(".vela/policies");
    std::fs::create_dir_all(&policy_dir).unwrap();
    std::fs::write(
        policy_dir.join("active.json"),
        serde_json::to_vec_pretty(&policy).unwrap(),
    )
    .unwrap();
    std::fs::write(
        policy_dir.join("active.sig.json"),
        serde_json::to_vec_pretty(&PolicySignatureRecord {
            policy_id: policy.id,
            signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
            signature: hex::encode(signature.to_bytes()),
            signed_at: signed_at.to_string(),
        })
        .unwrap(),
    )
    .unwrap();
}

fn one_json_object(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be exactly one JSON value: {error}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_land_rejected_without_git_change(
    dir: &Path,
    filename: &str,
    receipt: &serde_json::Value,
    context: &str,
) {
    let before_head = git_stdout(dir, &["rev-parse", "HEAD"]);
    let before_status = git_stdout(dir, &["status", "--porcelain"]);
    let path = dir.join(filename);
    std::fs::write(&path, serde_json::to_vec(receipt).unwrap()).unwrap();
    let landed = run(dir, &["land", filename, "--as", "agent:t", "--json"]);
    assert!(
        !landed.status.success(),
        "{context} unexpectedly landed\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&landed.stdout),
        String::from_utf8_lossy(&landed.stderr)
    );
    std::fs::remove_file(path).unwrap();
    assert_eq!(git_stdout(dir, &["rev-parse", "HEAD"]), before_head);
    assert_eq!(
        git_stdout(dir, &["status", "--porcelain"]),
        before_status,
        "{context} changed the frontier before rejection"
    );
}

fn refresh_receipt_binding(receipt: &mut serde_json::Value) {
    use serde_json::json;

    fn base64_standard(bytes: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let a = chunk[0];
            let b = chunk.get(1).copied().unwrap_or(0);
            let c = chunk.get(2).copied().unwrap_or(0);
            encoded.push(TABLE[(a >> 2) as usize] as char);
            encoded.push(TABLE[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
            encoded.push(if chunk.len() > 1 {
                TABLE[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char
            } else {
                '='
            });
            encoded.push(if chunk.len() > 2 {
                TABLE[(c & 0x3f) as usize] as char
            } else {
                '='
            });
        }
        encoded
    }

    let mut body = receipt.as_object().unwrap().clone();
    body.remove("attestation");
    let body_root =
        vela_protocol::canonical::sha256_canonical(&serde_json::Value::Object(body)).unwrap();
    let machine = receipt["machine"].clone();
    let statement = json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": machine["subject"].clone(),
        "predicateType": "https://vela.science/receipt/v1",
        "predicate": {
            "schema": "vela.receipt.predicate.v1",
            "machine": machine,
            "acceptance": receipt["acceptance"].clone(),
            "distillation": receipt["distillation"].clone(),
            "lineage": receipt["lineage"].clone(),
            "contributors": receipt["contributors"].clone(),
            "signature_identities": receipt["signature_identities"].clone(),
            "provenance": receipt["provenance"].clone(),
            "vela:receipt_body": {"sha256": body_root},
        }
    });
    let payload = vela_protocol::canonical::to_canonical_bytes(&statement).unwrap();
    receipt["attestation"]["statement"] = statement;
    receipt["attestation"]["dsse_envelope"]["payload"] =
        serde_json::Value::String(base64_standard(&payload));
}

fn snapshot_scientific_tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn collect(root: &Path, path: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        if !path.exists() {
            return;
        }
        let relative = path.strip_prefix(root).unwrap();
        let mut components = relative.components();
        if components
            .next()
            .is_some_and(|part| part.as_os_str() == ".vela")
            && components.next().is_some_and(|part| {
                matches!(
                    part.as_os_str().to_str(),
                    Some(
                        "agents"
                            | "keys"
                            | "operation-journals"
                            | "tasks"
                            | "work"
                            | "workspaces"
                            | "source-inbox"
                            | "artifact-blobs"
                    )
                )
            })
        {
            return;
        }
        if path.is_dir() {
            let mut entries = std::fs::read_dir(path)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect::<Vec<_>>();
            entries.sort();
            for entry in entries {
                collect(root, &entry, out);
            }
        } else {
            out.push((relative.display().to_string(), std::fs::read(path).unwrap()));
        }
    }

    let mut out = Vec::new();
    for relative in [".vela", "records", "frontier.json", "vela.lock", "proof"] {
        collect(dir, &dir.join(relative), &mut out);
    }
    out.sort_by(|left, right| left.0.cmp(&right.0));
    out
}

#[test]
fn json_mode_writes_one_object_only() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "one-object JSON publication regression",
    );

    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&output, "land with publication");
    let value = one_json_object(&output);
    assert_eq!(value["ok"], true);
    assert_eq!(value["command"], "land");
    assert!(value["operation_id"].as_str().is_some(), "{value}");
    assert!(value.get("publication").is_some(), "{value}");
}

#[test]
fn exact_receipt_retry_is_idempotent_across_frontier_and_git() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "an exact normalized receipt retries without a second transition",
    );

    let first = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&first, "first exact landing");
    let first = one_json_object(&first);
    assert_eq!(first["route"], "deferred", "{first}");

    let frontier_before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    let status_before = git_stdout(
        tmp.path(),
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );

    let retry = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&retry, "exact retry");
    let retry = one_json_object(&retry);
    assert_eq!(retry["route"], "exact_retry", "{retry}");
    for key in [
        "operation_id",
        "receipt_root",
        "record_id",
        "proposal_id",
        "finding_id",
    ] {
        assert_eq!(retry[key], first[key], "exact retry changed {key}");
    }
    assert_eq!(
        retry["publication"]["state"], "committed_local",
        "exact retry must recover or recognize the original publication: {retry}"
    );
    assert_eq!(
        retry["publication"]["commit"], first["publication"]["commit"],
        "exact retry minted a second publication commit: {retry}"
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), frontier_before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        status_before
    );
}

#[test]
fn completed_scientific_retry_resumes_git_publication() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "a completed scientific transaction retains resumable publication intent",
    );
    let baseline = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);

    let scientific_only = run_with_env(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
        &[("VELA_NO_PUBLISH", "1")],
    );
    assert_success(&scientific_only, "scientific-only landing");
    let scientific_only = one_json_object(&scientific_only);
    assert_eq!(scientific_only["publication"]["state"], "uncommitted");
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), baseline);

    let resumed = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&resumed, "resume exact publication");
    let resumed = one_json_object(&resumed);
    assert_eq!(resumed["route"], "exact_retry", "{resumed}");
    assert_eq!(resumed["publication"]["state"], "committed_local");
    let published = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    assert_ne!(published, baseline);
    assert_eq!(
        resumed["publication"]["commit"].as_str().unwrap(),
        published.trim()
    );

    let idempotent = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&idempotent, "recognize resumed publication");
    let idempotent = one_json_object(&idempotent);
    assert_eq!(
        idempotent["publication"]["commit"],
        resumed["publication"]["commit"]
    );
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), published);
}

#[test]
fn old_exact_retry_recovers_original_commit_after_later_land_and_in_clean_clone() {
    let producer = tempfile::TempDir::new().unwrap();
    init_git_frontier(producer.path());
    write_receipt_with_artifact(
        producer.path(),
        "receipt-a.json",
        "first exact publication remains attributable after later frontier state",
        "w-a.json",
        br#"{"witness":"a"}"#,
    );
    let receipt_a_bytes = std::fs::read(producer.path().join("receipt-a.json")).unwrap();
    let first = run(
        producer.path(),
        &["land", "receipt-a.json", "--as", "agent:t", "--json"],
    );
    assert_success(&first, "land historical operation A");
    let first = one_json_object(&first);
    let commit_a = first["publication"]["commit"]
        .as_str()
        .expect("A publication commit")
        .to_string();
    std::fs::remove_file(producer.path().join("receipt-a.json")).unwrap();
    std::fs::remove_file(producer.path().join("artifacts/w-a.json")).unwrap();

    write_receipt_with_artifact(
        producer.path(),
        "receipt-b.json",
        "a later exact publication advances shared frontier projections",
        "w-b.json",
        br#"{"witness":"b"}"#,
    );
    let second = run(
        producer.path(),
        &["land", "receipt-b.json", "--as", "agent:t", "--json"],
    );
    assert_success(&second, "land later operation B");
    let second = one_json_object(&second);
    let commit_b = second["publication"]["commit"]
        .as_str()
        .unwrap_or_else(|| panic!("B publication commit: {second}"))
        .to_string();
    assert_ne!(commit_a, commit_b);
    std::fs::write(producer.path().join("receipt-a.json"), &receipt_a_bytes).unwrap();

    let retry = run(
        producer.path(),
        &["land", "receipt-a.json", "--as", "agent:t", "--json"],
    );
    assert_success(&retry, "retry historical A after B");
    let retry = one_json_object(&retry);
    assert_eq!(retry["route"], "exact_retry", "{retry}");
    assert_eq!(retry["publication"]["commit"], commit_a, "{retry}");
    assert_eq!(
        git_stdout(producer.path(), &["rev-parse", "HEAD"]).trim(),
        commit_b
    );

    let clone_parent = tempfile::TempDir::new().unwrap();
    let clone = clone_parent.path().join("clone");
    let clone_output = Command::new("git")
        .args(["clone", "-q", "--no-local"])
        .arg(producer.path())
        .arg(&clone)
        .output()
        .unwrap();
    assert_success(&clone_output, "clone journal-free frontier");
    assert!(!clone.join(".vela/operation-journals").exists());
    assert!(!clone.join(".git/vela/operation-journals").exists());
    std::fs::write(clone.join("receipt-a.json"), receipt_a_bytes).unwrap();
    let clone_before = snapshot_scientific_tree(&clone);

    let clone_retry = run(
        &clone,
        &["land", "receipt-a.json", "--as", "agent:t", "--json"],
    );
    assert_success(&clone_retry, "clean-clone retry of A");
    let clone_retry = one_json_object(&clone_retry);
    assert_eq!(clone_retry["route"], "exact_retry", "{clone_retry}");
    assert_eq!(clone_retry["publication"]["commit"], commit_a);
    for key in [
        "operation_id",
        "receipt_root",
        "record_id",
        "proposal_id",
        "finding_id",
    ] {
        assert_eq!(clone_retry[key], first[key], "clean clone changed {key}");
    }
    assert_eq!(snapshot_scientific_tree(&clone), clone_before);
    assert_eq!(git_stdout(&clone, &["rev-parse", "HEAD"]).trim(), commit_b);
}

#[test]
fn deny_has_zero_canonical_and_git_delta() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "a signed deny route must leave every durable boundary unchanged",
    );
    write_active_deny_policy(tmp.path());

    let scientific_before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    let index_before = git_stdout(tmp.path(), &["write-tree"]);
    let status_before = git_stdout(
        tmp.path(),
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );

    let denied = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert!(!denied.status.success(), "Deny must not report success");
    let denied = one_json_object(&denied);
    assert_eq!(denied["ok"], false, "{denied}");
    assert!(
        denied["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("policy denies")),
        "{denied}"
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), scientific_before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(git_stdout(tmp.path(), &["write-tree"]), index_before);
    assert_eq!(
        git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        status_before
    );
}

#[test]
fn external_public_artifact_requires_complete_descriptor() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "external.json",
        "an external public artifact needs enough metadata to remain inspectable offline",
    );
    let receipt_path = tmp.path().join("external.json");
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    let artifact = &mut receipt["artifacts"][0];
    artifact["path"] = serde_json::json!("https://example.invalid/result.bin");
    artifact["uri"] = serde_json::json!("https://example.invalid/result.bin");
    artifact.as_object_mut().unwrap().remove("size_bytes");
    artifact.as_object_mut().unwrap().remove("media_type");
    refresh_receipt_binding(&mut receipt);
    std::fs::write(
        &receipt_path,
        vela_protocol::canonical::to_canonical_bytes(&receipt).unwrap(),
    )
    .unwrap();
    let before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);

    let output = run(
        tmp.path(),
        &["land", "external.json", "--as", "agent:t", "--json"],
    );
    assert!(!output.status.success(), "incomplete descriptor must fail");
    let output = one_json_object(&output);
    assert!(
        output["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("explicit size_bytes descriptor")),
        "{output}"
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
}

#[test]
fn same_claim_new_evidence_is_not_an_exact_retry() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let claim = "the same scoped claim has an independent evidence submission";
    write_receipt_with_artifact_as(
        tmp.path(),
        "receipt-a.json",
        claim,
        "w-a.json",
        br#"{"witness":"first"}"#,
        "agent:replicator-a",
        0x31,
    );
    let first = run(
        tmp.path(),
        &[
            "land",
            "receipt-a.json",
            "--as",
            "agent:replicator-a",
            "--json",
        ],
    );
    assert_success(&first, "first evidence landing");
    let first = one_json_object(&first);

    write_receipt_with_artifact_as(
        tmp.path(),
        "receipt-b.json",
        claim,
        "w-b.json",
        br#"{"witness":"second"}"#,
        "agent:replicator-b",
        0x32,
    );
    let second = run(
        tmp.path(),
        &[
            "land",
            "receipt-b.json",
            "--as",
            "agent:replicator-b",
            "--json",
        ],
    );
    assert_success(&second, "second evidence landing");
    let second = one_json_object(&second);

    assert_ne!(second["route"], "exact_retry", "{second}");
    for key in [
        "operation_id",
        "receipt_root",
        "record_id",
        "proposal_id",
        "finding_id",
    ] {
        assert_ne!(
            second[key], first[key],
            "different evidence collapsed {key}: first={first}, second={second}"
        );
    }
    let first_proposal_id = first["proposal_id"].as_str().unwrap();
    let first_proposal: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            tmp.path()
                .join(".vela/proposals")
                .join(format!("{first_proposal_id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(first_proposal["actor"]["id"], "agent:replicator-a");

    let second_proposal_id = second["proposal_id"].as_str().unwrap();
    let second_proposal: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            tmp.path()
                .join(".vela/proposals")
                .join(format!("{second_proposal_id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(second_proposal["actor"]["id"], "agent:replicator-b");
    let related = second_proposal["payload"]["vela_submission"]["same_claim_findings"]
        .as_array()
        .unwrap_or_else(|| {
            panic!("second proposal must retain the same-claim relation: {second_proposal}")
        });
    assert_eq!(
        related,
        &[serde_json::Value::String(
            first["finding_id"].as_str().unwrap().to_string()
        )],
        "second evidence did not point back to the related first finding: {second_proposal}"
    );
}

#[test]
fn clean_clone_rebuilds_public_review_root_without_restricted_bytes() {
    use sha2::{Digest, Sha256};

    let producer = tempfile::TempDir::new().unwrap();
    init_git_frontier(producer.path());
    write_receipt_with_artifact(
        producer.path(),
        "privacy-receipt.json",
        "a mixed-disclosure receipt keeps its public review packet portable",
        "public-witness.json",
        br#"{"public":true}"#,
    );
    let restricted_bytes = b"VELA-RESTRICTED-OPENING-7f75f37b";
    let restricted_path = producer.path().join(".vela/work/privacy/secret.bin");
    std::fs::create_dir_all(restricted_path.parent().unwrap()).unwrap();
    std::fs::write(&restricted_path, restricted_bytes).unwrap();

    let receipt_path = producer.path().join("privacy-receipt.json");
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    receipt["artifacts"].as_array_mut().unwrap().extend([
        serde_json::json!({
            "path": "https://example.invalid/frozen-dataset.bin",
            "uri": "https://example.invalid/frozen-dataset.bin",
            "kind": "dataset",
            "sha256": "9".repeat(64),
            "size_bytes": 128,
            "media_type": "application/octet-stream",
            "locator_integrity": "immutable",
            "availability": "unknown"
        }),
        serde_json::json!({
            "path": "opaque:custodian-fixture-7",
            "kind": "restricted_witness",
            "disclosure": "restricted",
            "locator_integrity": "unknown",
            "availability": "available"
        }),
    ]);
    receipt["x:portable-review"] = serde_json::json!({
        "distiller": "outside-producer-fixture",
        "belief": {"value": 0.61, "attributed_to": "agent:t"}
    });
    receipt["machine"]["subject"] = serde_json::Value::Array(
        receipt["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|artifact| {
                let mut subject = serde_json::Map::new();
                subject.insert("name".to_string(), artifact["path"].clone());
                if artifact
                    .get("disclosure")
                    .and_then(serde_json::Value::as_str)
                    != Some("restricted")
                {
                    if let Some(digest) = artifact.get("sha256") {
                        subject.insert("digest".to_string(), serde_json::json!({"sha256": digest}));
                    }
                    if let Some(uri) = artifact.get("uri") {
                        subject.insert("uri".to_string(), uri.clone());
                    }
                }
                serde_json::Value::Object(subject)
            })
            .collect(),
    );
    refresh_receipt_binding(&mut receipt);

    let restricted_digest = hex::encode(Sha256::digest(restricted_bytes));
    let restricted_location = restricted_path.to_string_lossy().into_owned();
    let mut descriptor_leak = receipt.clone();
    descriptor_leak["artifacts"][2]["sha256"] =
        serde_json::Value::String(restricted_digest.clone());
    refresh_receipt_binding(&mut descriptor_leak);
    assert_land_rejected_without_git_change(
        producer.path(),
        "malicious-descriptor.receipt.json",
        &descriptor_leak,
        "restricted descriptor digest leak",
    );

    let mut mirror_leak = receipt.clone();
    mirror_leak["machine"]["subject"][2]["digest"] =
        serde_json::json!({"sha256": restricted_digest});
    mirror_leak["machine"]["subject"][2]["uri"] =
        serde_json::Value::String(restricted_location.clone());
    refresh_receipt_binding(&mut mirror_leak);
    assert_land_rejected_without_git_change(
        producer.path(),
        "malicious-subject.receipt.json",
        &mirror_leak,
        "restricted subject mirror leak",
    );

    let mut prov_leak = receipt.clone();
    prov_leak["attestation"]["prov"] = serde_json::json!({
        "entity": {
            "artifact:opaque:custodian-fixture-7": {
                "prov:type": "vela:artifact",
                "vela:kind": "restricted_witness",
                "opening": "VELA-RESTRICTED-OPENING-7f75f37b",
                "location": restricted_location,
            }
        }
    });
    assert_land_rejected_without_git_change(
        producer.path(),
        "malicious-prov.receipt.json",
        &prov_leak,
        "restricted PROV mirror leak",
    );

    let receipt = vela_protocol::receipt_v1::ReceiptV1::parse(
        &vela_protocol::canonical::to_canonical_bytes(&receipt).unwrap(),
    )
    .unwrap();
    std::fs::write(&receipt_path, receipt.canonical_bytes().unwrap()).unwrap();

    let landed = run(
        producer.path(),
        &["land", "privacy-receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&landed, "mixed-disclosure landing");
    let landed = one_json_object(&landed);
    assert_eq!(landed["route"], "deferred", "{landed}");
    assert_eq!(
        landed["publication"]["state"], "committed_local",
        "{landed}"
    );

    let record_id = landed["record_id"].as_str().unwrap();
    let record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            producer
                .path()
                .join("records")
                .join(format!("{record_id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    let artifacts = record["artifacts"].as_array().unwrap();
    assert!(artifacts.iter().any(|artifact| {
        artifact["disclosure"] == "restricted"
            && artifact["locator"] == "opaque:custodian-fixture-7"
            && artifact.get("sha256").is_none()
    }));
    assert!(artifacts.iter().any(|artifact| {
        artifact["kind"] == "dataset"
            && artifact["locator_integrity"] == "immutable"
            && artifact.get("availability").is_none()
    }));
    let proposal_id = landed["proposal_id"].as_str().unwrap();
    let proposal: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            producer
                .path()
                .join(".vela/proposals")
                .join(format!("{proposal_id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    let review_material_path = proposal["payload"]["vela_submission"]["review_material_path"]
        .as_str()
        .expect("proposal must retain its public review material path");
    let review_material_bytes = std::fs::read(producer.path().join(review_material_path)).unwrap();
    let review_material: serde_json::Value =
        serde_json::from_slice(&review_material_bytes).unwrap();
    assert_eq!(review_material["proposal_id"], proposal_id);
    assert_eq!(review_material["receipt_root"], landed["receipt_root"]);
    assert_eq!(review_material["route"]["policy_state"], "closed");
    assert!(review_material["route"]["policy_decision"].is_null());
    assert_eq!(review_material["route"]["engine_gate"]["strict"], true);
    let review_material_root_before =
        vela_protocol::canonical::sha256_canonical(&review_material).unwrap();
    let review_root_before = vela_protocol::canonical::sha256_canonical(&serde_json::json!({
        "receipt_root": landed["receipt_root"],
        "artifacts": artifacts,
    }))
    .unwrap();

    // A genuinely independent producer may submit the same claim with new
    // evidence. Both public receipt/review roots must survive the clone; this
    // is replication, never exact-retry deduplication.
    std::fs::remove_file(producer.path().join("artifacts/public-witness.json")).unwrap();
    let replicated_claim = "a mixed-disclosure receipt keeps its public review packet portable";
    write_receipt_with_artifact_as(
        producer.path(),
        "replication-receipt.json",
        replicated_claim,
        "replication-witness.json",
        br#"{"public":"independent replication"}"#,
        "agent:replicator-b",
        0x52,
    );
    let replicated = run(
        producer.path(),
        &[
            "land",
            "replication-receipt.json",
            "--as",
            "agent:replicator-b",
            "--json",
        ],
    );
    assert_success(&replicated, "independent replication landing");
    let replicated = one_json_object(&replicated);
    assert_ne!(replicated["route"], "exact_retry", "{replicated}");
    assert_ne!(replicated["receipt_root"], landed["receipt_root"]);
    assert_ne!(replicated["proposal_id"], landed["proposal_id"]);
    let replicated_proposal_id = replicated["proposal_id"].as_str().unwrap();
    let replicated_proposal: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            producer
                .path()
                .join(".vela/proposals")
                .join(format!("{replicated_proposal_id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(replicated_proposal["actor"]["id"], "agent:replicator-b");
    assert_eq!(
        replicated_proposal["payload"]["vela_submission"]["same_claim_findings"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    let replicated_review_path =
        replicated_proposal["payload"]["vela_submission"]["review_material_path"]
            .as_str()
            .unwrap()
            .to_string();
    let replicated_review_root = vela_protocol::canonical::sha256_canonical(
        &serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(producer.path().join(&replicated_review_path)).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let clone_parent = tempfile::TempDir::new().unwrap();
    let clone = clone_parent.path().join("clone");
    let clone_output = Command::new("git")
        .args([
            "clone",
            "--quiet",
            "--no-local",
            producer.path().to_str().unwrap(),
            clone.to_str().unwrap(),
        ])
        .env("HOME", clone_parent.path())
        .output()
        .unwrap();
    assert_success(&clone_output, "clean clone");
    std::fs::remove_dir_all(producer.path()).unwrap();

    let receipt_hex = landed["receipt_root"]
        .as_str()
        .unwrap()
        .strip_prefix("sha256:")
        .unwrap();
    let cloned_receipt = vela_protocol::receipt_v1::ReceiptV1::parse(
        &std::fs::read(
            clone
                .join("records/receipts/sha256")
                .join(format!("{receipt_hex}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        cloned_receipt.canonical_root().unwrap(),
        landed["receipt_root"].as_str().unwrap()
    );
    let cloned_record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(clone.join("records").join(format!("{record_id}.json"))).unwrap(),
    )
    .unwrap();
    let cloned_review_material: serde_json::Value =
        serde_json::from_slice(&std::fs::read(clone.join(review_material_path)).unwrap()).unwrap();
    assert_eq!(
        vela_protocol::canonical::sha256_canonical(&cloned_review_material).unwrap(),
        review_material_root_before,
        "clean clone changed the staged decision and gate facts"
    );
    let review_root_after = vela_protocol::canonical::sha256_canonical(&serde_json::json!({
        "receipt_root": cloned_receipt.canonical_root().unwrap(),
        "artifacts": cloned_record["artifacts"],
    }))
    .unwrap();
    assert_eq!(review_root_after, review_root_before);
    let replicated_hex = replicated["receipt_root"]
        .as_str()
        .unwrap()
        .strip_prefix("sha256:")
        .unwrap();
    let replicated_receipt = vela_protocol::receipt_v1::ReceiptV1::parse(
        &std::fs::read(
            clone
                .join("records/receipts/sha256")
                .join(format!("{replicated_hex}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        replicated_receipt.canonical_root().unwrap(),
        replicated["receipt_root"].as_str().unwrap()
    );
    let cloned_replicated_proposal: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            clone
                .join(".vela/proposals")
                .join(format!("{replicated_proposal_id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        cloned_replicated_proposal["actor"]["id"],
        "agent:replicator-b"
    );
    let cloned_replicated_review: serde_json::Value =
        serde_json::from_slice(&std::fs::read(clone.join(&replicated_review_path)).unwrap())
            .unwrap();
    assert_eq!(
        vela_protocol::canonical::sha256_canonical(&cloned_replicated_review).unwrap(),
        replicated_review_root
    );

    let objects = Command::new("git")
        .arg("-C")
        .arg(&clone)
        .args([
            "cat-file",
            "--batch-all-objects",
            "--batch-check=%(objectname) %(objecttype)",
        ])
        .output()
        .unwrap();
    assert_success(&objects, "enumerate clone objects");
    let forbidden_markers = [
        restricted_bytes.as_slice(),
        restricted_digest.as_bytes(),
        restricted_location.as_bytes(),
        b".vela/work/privacy/secret.bin".as_slice(),
    ];
    for line in String::from_utf8(objects.stdout).unwrap().lines() {
        let mut fields = line.split_whitespace();
        let Some(oid) = fields.next() else { continue };
        if fields.next() != Some("blob") {
            continue;
        }
        let blob = Command::new("git")
            .arg("-C")
            .arg(&clone)
            .args(["cat-file", "blob", oid])
            .output()
            .unwrap();
        assert_success(&blob, "read clone blob");
        for marker in forbidden_markers {
            assert!(
                !blob
                    .stdout
                    .windows(marker.len())
                    .any(|window| window == marker),
                "restricted bytes, digest, opening, or location entered Git object {oid}"
            );
        }
    }
}

#[test]
fn flag_authored_land_closes_only_its_exact_private_work_session() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::create_dir_all(tmp.path().join("artifacts")).unwrap();
    std::fs::write(
        tmp.path().join("artifacts/session-witness.json"),
        br#"{"session":true}"#,
    )
    .unwrap();
    let agent_key = "42".repeat(32);
    let env = [("VELA_AGENT_KEY_HEX", agent_key.as_str())];

    let work = run_with_env(
        tmp.path(),
        &["work", "erdos:session-close", "--as", "agent:t", "--json"],
        &env,
    );
    assert_success(&work, "open work session");
    let work = one_json_object(&work);
    let session = tmp.path().join(".vela/work/erdos-session-close");
    let offer = session.join("offer.json");
    let completed = session.join("landed.json");
    assert_eq!(work["target"], "erdos:session-close", "{work}");
    assert!(offer.is_file());
    std::fs::write(session.join("producer-notes.txt"), "keep this scratch\n").unwrap();

    let land = run_with_env(
        tmp.path(),
        &[
            "land",
            "--claim",
            "the exact work-session receipt closes its private offer",
            "--type",
            "computational",
            "--replayability",
            "exact",
            "--artifact",
            "artifacts/session-witness.json:witness",
            "--caveat",
            "fixture evidence only",
            "--as",
            "agent:t",
            "--json",
        ],
        &env,
    );
    assert_success(&land, "land flag-authored work-session receipt");
    let land = one_json_object(&land);
    assert!(!offer.exists(), "the exact completed offer must be retired");
    assert!(
        completed.is_file(),
        "completion metadata must remain private"
    );
    assert_eq!(
        std::fs::read_to_string(session.join("producer-notes.txt")).unwrap(),
        "keep this scratch\n",
        "landing must preserve unrelated producer scratch"
    );
    let completed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(completed).unwrap()).unwrap();
    assert_eq!(
        completed["schema"],
        "vela.work-session-completed.internal.v1"
    );
    assert_eq!(completed["target"], "erdos:session-close");
    for key in ["operation_id", "receipt_root", "record_id", "proposal_id"] {
        assert_eq!(completed[key], land[key], "completion changed {key}");
    }

    let committed_paths = git_stdout(
        tmp.path(),
        &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
    );
    assert!(
        !committed_paths
            .lines()
            .any(|path| path.starts_with(".vela/work/")),
        "private work coordination entered the Git publication: {committed_paths}"
    );
}

#[test]
fn receipt_with_a_different_key_cannot_close_an_agents_work_session() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let lease_key = "42".repeat(32);
    let env = [("VELA_AGENT_KEY_HEX", lease_key.as_str())];
    let work = run_with_env(
        tmp.path(),
        &["work", "erdos:session-owner", "--as", "agent:t", "--json"],
        &env,
    );
    assert_success(&work, "open protected work session");
    one_json_object(&work);
    let offer = tmp.path().join(".vela/work/erdos-session-owner/offer.json");

    write_receipt_with_artifact_as(
        tmp.path(),
        "wrong-key.json",
        "a self-signed receipt cannot retire another key's coordination state",
        "wrong-key-witness.json",
        br#"{"wrong_key":true}"#,
        "agent:t",
        0x43,
    );
    let receipt_path = tmp.path().join("wrong-key.json");
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    receipt["environment"]["vela:producer_context"]["base_path"] =
        serde_json::json!(".vela/work/erdos-session-owner");
    refresh_receipt_binding(&mut receipt);
    std::fs::write(
        &receipt_path,
        vela_protocol::canonical::to_canonical_bytes(&receipt).unwrap(),
    )
    .unwrap();
    let before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);

    let output = run_with_env(
        tmp.path(),
        &["land", "wrong-key.json", "--as", "agent:t", "--json"],
        &env,
    );
    assert!(!output.status.success(), "wrong-key close must fail");
    let output = one_json_object(&output);
    assert!(
        output["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("does not match the target lease key")),
        "{output}"
    );
    assert!(
        offer.is_file(),
        "failed close must preserve the active offer"
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
}

#[test]
fn invalid_land_human_output_reports_zero_delta_and_safe_next_command() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);

    let output = run_with_env(
        tmp.path(),
        &[
            "land",
            "--claim",
            "invalid artifact input changes nothing",
            "--artifact",
            "artifacts/missing.json:witness",
            "--caveat",
            "fixture evidence only",
            "--as",
            "agent:t",
        ],
        &[("VELA_ADVICE", "1")],
    );
    assert!(!output.status.success(), "missing artifact must fail");
    assert!(
        output.stdout.is_empty(),
        "human error stdout must stay empty"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unchanged"), "{stderr}");
    assert!(
        stderr.contains("canonical Vela state, Git refs, index, and worktree"),
        "{stderr}"
    );
    assert!(stderr.contains("retained"), "{stderr}");
    assert!(stderr.contains("vop_"), "{stderr}");
    assert!(stderr.contains("next"), "{stderr}");
    assert!(stderr.contains("vela land --help"), "{stderr}");
    assert_eq!(snapshot_scientific_tree(tmp.path()), before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
}

#[test]
fn concurrent_publication_owner_blocks_scientific_mutation() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "busy publication must not race a scientific write",
    );

    let lock_path = tmp.path().join(".git/vela/publication.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let publication_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .unwrap();
    publication_lock.lock().unwrap();

    let before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert!(!output.status.success(), "busy land must retry, not mutate");
    let value = one_json_object(&output);
    assert_eq!(value["ok"], false, "{value}");
    assert!(
        value["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("scientific state was not changed")),
        "{value}"
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
}

#[test]
fn failed_push_reports_committed_local() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "failed push retains local publication",
    );

    let output = run(
        tmp.path(),
        &[
            "land",
            "receipt.json",
            "--as",
            "agent:t",
            "--push",
            "--json",
        ],
    );
    assert_success(&output, "land despite push failure");
    let value = one_json_object(&output);
    assert_eq!(value["publication"]["state"], "committed_local", "{value}");
    assert!(value["publication"]["commit"].as_str().is_some(), "{value}");
    assert!(
        value["publication"]["recovery_command"]
            .as_str()
            .is_some_and(|command| {
                command.starts_with("vela publication recover --operation vop_")
                    && command.ends_with(" --push")
            }),
        "{value}"
    );
}

#[test]
fn failed_push_human_output_separates_request_from_publication_recovery() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "failed push human output has distinct identities",
    );

    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--push"],
    );
    assert_success(&output, "human land despite push failure");
    assert!(output.stderr.is_empty(), "unexpected stderr");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let request = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("request"))
        .expect("request line");
    let retained = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("retained"))
        .expect("retained line");
    let remote = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("remote"))
        .expect("remote line");
    let next = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("next"))
        .expect("next line");
    assert!(request.contains("vop_"), "{stdout}");
    assert!(retained.contains("local commit"), "{stdout}");
    assert!(!retained.contains("vop_"), "{stdout}");
    assert!(remote.contains("unverified"), "{stdout}");
    assert!(
        next.contains("vela publication recover --operation vop_") && next.ends_with(" --push"),
        "{stdout}"
    );
}

#[test]
fn publication_never_commits_callers_index() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::write(tmp.path().join("notes.txt"), "baseline\n").unwrap();
    assert_success(
        &git(tmp.path(), &["add", "notes.txt"]),
        "stage notes baseline",
    );
    assert_success(
        &git(tmp.path(), &["commit", "-qm", "notes baseline"]),
        "commit notes baseline",
    );

    std::fs::write(tmp.path().join("notes.txt"), "caller staged bytes\n").unwrap();
    assert_success(
        &git(tmp.path(), &["add", "notes.txt"]),
        "stage caller bytes",
    );
    write_receipt(tmp.path(), "receipt.json", "path-scoped publication");
    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&output, "path-scoped land");
    let value = one_json_object(&output);
    assert_eq!(value["publication"]["state"], "committed_local", "{value}");

    let committed_paths = git_stdout(
        tmp.path(),
        &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
    );
    assert!(
        !committed_paths.lines().any(|path| path == "notes.txt"),
        "publication captured the caller's staged file: {committed_paths}"
    );
    assert_eq!(
        git_stdout(tmp.path(), &["show", ":notes.txt"]),
        "caller staged bytes\n",
        "publication changed the caller's staged entry"
    );
    assert_eq!(
        git_stdout(tmp.path(), &["show", "HEAD:notes.txt"]),
        "baseline\n",
        "publication committed unrelated staged bytes"
    );
}

#[test]
fn publication_preserves_unstaged_work() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::write(tmp.path().join("notes.txt"), "baseline\n").unwrap();
    assert_success(
        &git(tmp.path(), &["add", "notes.txt"]),
        "stage notes baseline",
    );
    assert_success(
        &git(tmp.path(), &["commit", "-qm", "notes baseline"]),
        "commit notes baseline",
    );

    std::fs::write(tmp.path().join("notes.txt"), "caller unstaged bytes\n").unwrap();
    std::fs::write(tmp.path().join("scratch.txt"), "caller untracked bytes\n").unwrap();
    write_receipt(tmp.path(), "receipt.json", "preserve unstaged publication");
    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&output, "land around unstaged work");
    one_json_object(&output);

    assert_eq!(
        std::fs::read_to_string(tmp.path().join("notes.txt")).unwrap(),
        "caller unstaged bytes\n"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("scratch.txt")).unwrap(),
        "caller untracked bytes\n"
    );
    let status = git_stdout(tmp.path(), &["status", "--porcelain=v1"]);
    assert!(
        status.lines().any(|line| line == " M notes.txt"),
        "{status}"
    );
    assert!(
        status.lines().any(|line| line == "?? scratch.txt"),
        "{status}"
    );
}

#[test]
fn publication_refuses_overlapping_vela_edits() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::create_dir_all(tmp.path().join("sources")).unwrap();
    let caller_source = tmp.path().join("sources/preexisting.txt");
    std::fs::write(&caller_source, "baseline source\n").unwrap();
    assert_success(
        &git(tmp.path(), &["add", "sources/preexisting.txt"]),
        "stage source baseline",
    );
    assert_success(
        &git(tmp.path(), &["commit", "-qm", "source baseline"]),
        "commit source baseline",
    );
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    std::fs::write(
        &caller_source,
        "baseline source\ncaller-owned pre-existing edit\n",
    )
    .unwrap();
    write_receipt(
        tmp.path(),
        "receipt.json",
        "scientific write survives publication overlap refusal",
    );

    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&output, "land despite publication overlap refusal");
    let value = one_json_object(&output);
    assert_eq!(value["publication"]["state"], "uncommitted", "{value}");
    assert!(
        value["publication"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("pre-existing unstaged Vela edit")),
        "{value}"
    );
    assert_eq!(
        git_stdout(tmp.path(), &["rev-parse", "HEAD"]),
        head_before,
        "preflight refusal must leave HEAD unchanged"
    );
    let proposal_id = value["proposal_id"].as_str().unwrap();
    assert!(
        tmp.path()
            .join(".vela/proposals")
            .join(format!("{proposal_id}.json"))
            .is_file(),
        "scientific landing must remain durable when Git publication refuses"
    );
    assert!(
        std::fs::read_to_string(caller_source)
            .unwrap()
            .contains("caller-owned pre-existing edit")
    );
}

#[cfg(unix)]
#[test]
fn publication_bypasses_all_repository_hooks() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let hooks = tmp.path().join(".git/hooks");
    let marker = tmp.path().join("hook-ran.marker");
    let quoted_marker = marker.display().to_string().replace('\'', "'\\''");
    let body = format!("#!/bin/sh\nprintf hook-ran > '{quoted_marker}'\nexit 91\n");
    for name in [
        "pre-commit",
        "prepare-commit-msg",
        "commit-msg",
        "post-commit",
        "reference-transaction",
        "post-rewrite",
    ] {
        let path = hooks.join(name);
        std::fs::write(&path, &body).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    write_receipt(tmp.path(), "receipt.json", "hooks are not authority");
    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&output, "hook-bypassing publication");
    one_json_object(&output);
    assert!(
        !marker.exists(),
        "repository hook ran during Vela publication"
    );
}

#[cfg(unix)]
#[test]
fn publication_scrubs_inherited_git_config_injection() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let injected_hooks = tmp.path().join("injected-hooks");
    std::fs::create_dir(&injected_hooks).unwrap();
    let marker = tmp.path().join("injected-hook-ran.marker");
    let quoted_marker = marker.display().to_string().replace('\'', "'\\''");
    let hook = injected_hooks.join("reference-transaction");
    std::fs::write(
        &hook,
        format!("#!/bin/sh\nprintf hook-ran > '{quoted_marker}'\nexit 91\n"),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&hook, permissions).unwrap();

    write_receipt(
        tmp.path(),
        "receipt.json",
        "inherited Git config is not publication authority",
    );
    let hooks_value = injected_hooks.to_string_lossy().into_owned();
    let output = run_with_env(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
        &[
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "core.hooksPath"),
            ("GIT_CONFIG_VALUE_0", hooks_value.as_str()),
        ],
    );
    assert_success(&output, "publication under hostile inherited Git config");
    let value = one_json_object(&output);
    assert_eq!(value["publication"]["state"], "committed_local", "{value}");
    assert!(
        !marker.exists(),
        "an inherited GIT_CONFIG_* hook ran during publication"
    );
}

#[test]
fn mcp_profiles_expose_no_finalizer() {
    use vela_edge::tool_registry::{McpProfile, get_tool, tools_for_profile};

    assert!(get_tool("decide").is_none());
    for profile in [
        McpProfile::ReadOnly,
        McpProfile::Draft,
        McpProfile::Maintainer,
    ] {
        assert!(
            tools_for_profile(profile)
                .iter()
                .all(|tool| tool.name != "decide"),
            "removed finalizer leaked into {} MCP discovery",
            profile.as_str()
        );
    }
    let draft = tools_for_profile(McpProfile::Draft)
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    let maintainer = tools_for_profile(McpProfile::Maintainer)
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    assert_eq!(maintainer, draft, "maintainer must only alias draft");
}

#[test]
fn untrusted_terminal_text_is_escaped() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert_success(
        &run(
            tmp.path(),
            &["init", ".", "--name", "safe-text", "--no-git", "--json"],
        ),
        "init frontier",
    );
    let receipt = serde_json::json!({
        "schema": "vela.receipt.v1",
        "claim": "safe failure path",
        "type": "computational",
        "replayability": "bad\u{001b}]8;;https://bad.example\u{0007}\u{202e}",
        "caveats": ["fixture"],
    });
    std::fs::write(
        tmp.path().join("receipt.json"),
        serde_json::to_vec(&receipt).unwrap(),
    )
    .unwrap();

    let output = run(tmp.path(), &["land", "receipt.json", "--as", "agent:t"]);
    assert!(!output.status.success(), "hostile receipt must be rejected");
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!rendered.contains('\u{001b}'), "{rendered:?}");
    assert!(!rendered.contains('\u{0007}'), "{rendered:?}");
    assert!(!rendered.contains('\u{202e}'), "{rendered:?}");
    assert!(rendered.contains("\\u{001B}"), "{rendered:?}");
    assert!(rendered.contains("\\u{202E}"), "{rendered:?}");
}

#[test]
fn strict_check_surfaces_policy_lane_replay_in_human_and_json_output() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::remove_file(tmp.path().join(".vela/keys/t/private.key")).unwrap();
    let mut frontier = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    frontier
        .events
        .push(vela_protocol::events::new_finding_event(
            vela_protocol::events::FindingEventInput {
                kind: vela_protocol::events::EVENT_KIND_ATTESTATION_RECORDED,
                finding_id: "vf_policy_lane_fixture",
                actor_id: "agent:t",
                actor_type: "agent",
                reason: "malformed strict-check fixture",
                before_hash: vela_protocol::events::NULL_HASH,
                after_hash: vela_protocol::events::NULL_HASH,
                payload: serde_json::json!({
                    vela_protocol::proposals::policy_accept::POLICY_LANE_PAYLOAD_KEY: {
                        "schema": "vela.policy-lane.v2",
                        "policy_id": "vap_forged"
                    }
                }),
                caveats: Vec::new(),
                timestamp: Some("2026-07-14T00:00:00Z"),
            },
        ));
    vela_protocol::repo::save_to_path(tmp.path(), &frontier).unwrap();

    let json_output = run(tmp.path(), &["check", ".", "--strict", "--json"]);
    assert!(
        !json_output.status.success(),
        "malformed policy lane must fail strict JSON check"
    );
    let payload = one_json_object(&json_output);
    let policy_lane = payload["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "policy_lane")
        .expect("policy_lane check entry");
    assert_eq!(policy_lane["status"], "fail", "{payload}");
    assert_eq!(policy_lane["failed"], 1, "{payload}");
    assert!(
        payload["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item["rule_id"] == "policy_lane_replay"
                    && item["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("malformed or open-ended"))
            })
    );

    let human_output = run(tmp.path(), &["check", ".", "--strict"]);
    assert!(
        !human_output.status.success(),
        "malformed policy lane must fail strict human check"
    );
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&human_output.stdout),
        String::from_utf8_lossy(&human_output.stderr)
    );
    assert!(
        rendered.contains("policy-lane replay: 1 conflict(s)"),
        "{rendered}"
    );
    assert!(rendered.contains("malformed or open-ended"), "{rendered}");
}
