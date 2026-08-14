use std::fs;
use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use vela_protocol::authority::{
    AuthorityEnvelopeV1, AuthorityEventV1, AuthorityKeysetV1, AuthorityRecordV1, ObjectDeltaV1,
};
use vela_protocol::authority_history::{
    AuthorityHistoryEra, AuthorityHistoryInput, AuthorityHistoryVerification,
    verify_authority_history,
};
use vela_protocol::authorization::AuthorizationModelV1;
use vela_protocol::canonical::{from_json_slice_strict, sha256_canonical, to_canonical_bytes};
use vela_protocol::repository::RepositoryV4;
use vela_protocol::repository_origin::RepositoryOriginV1;

const FIXTURE_ID: &str = "math-coh-00";
const EMPTY_READ_ROOT: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const KEYSET_PATH: &str =
    "authority/keysets/43762983e444ec2a8b7fc906b05e45d66e9af469024db0691324707d5306deb1.json";
const MODEL_PATH: &str =
    "authority/models/4d6d5e283577dde287b743871a67cb84bf1346d61ad8786c320d70f1af7e8965.json";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustAnchor {
    schema: String,
    repository_id: String,
    first_authority_record_root: String,
}

struct Fixture {
    expected: Value,
    anchor: TrustAnchor,
    history: History,
    records: Vec<AuthorityRecordV1>,
}

#[derive(Clone)]
struct History {
    keysets: Vec<AuthorityKeysetV1>,
    models: Vec<AuthorizationModelV1>,
    events: Vec<AuthorityEventV1>,
    envelopes: Vec<AuthorityEnvelopeV1>,
}

impl Fixture {
    fn load() -> Self {
        let expected: Value = read_strict("expected.json");
        let anchor = read_strict("trust-anchor.json");
        let keysets: Vec<AuthorityKeysetV1> = vec![read_strict(KEYSET_PATH)];
        let models: Vec<AuthorizationModelV1> = vec![read_strict(MODEL_PATH)];
        keysets[0].validate().unwrap();
        models[0].validate().unwrap();

        let mut envelopes = Vec::new();
        let mut records = Vec::new();
        let mut events = Vec::new();
        for item in array(&expected, "/records") {
            let bytes = read(text(item, "/path"));
            let envelope: AuthorityEnvelopeV1 = from_json_slice_strict(&bytes).unwrap();
            let payload = BASE64_STANDARD.decode(&envelope.payload).unwrap();
            let record: AuthorityRecordV1 = from_json_slice_strict(&payload).unwrap();
            assert_eq!(to_canonical_bytes(&record).unwrap(), payload);
            for event_id in array(item, "/event_ids") {
                let path = format!("authority/events/{}.json", event_id.as_str().unwrap());
                let event: AuthorityEventV1 = read_strict(&path);
                event.validate().unwrap();
                events.push(event);
            }
            envelopes.push(envelope);
            records.push(record);
        }
        Self {
            expected,
            anchor,
            history: History {
                keysets,
                models,
                events,
                envelopes,
            },
            records,
        }
    }

    fn verify_with(&self, history: &History) -> Result<AuthorityHistoryVerification, String> {
        verify_authority_history(AuthorityHistoryInput {
            repository_id: text(&self.expected, "/repository_id"),
            initial_event_log_root: text(&self.expected, "/initial_event_log_root"),
            initial_actor_registry_root: text(&self.expected, "/initial_actor_registry_root"),
            authority_keysets: &history.keysets,
            authorization_models: &history.models,
            authority_events: &history.events,
            authority_envelopes: &history.envelopes,
        })
    }

    fn verify(&self) -> AuthorityHistoryVerification {
        self.verify_with(&self.history).unwrap()
    }
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/authority")
        .join(FIXTURE_ID)
}

fn read(relative: &str) -> Vec<u8> {
    fs::read(fixture_root().join(relative)).unwrap()
}

fn read_strict<T: DeserializeOwned>(relative: &str) -> T {
    from_json_slice_strict(&read(relative)).unwrap()
}

fn text<'a>(value: &'a Value, pointer: &str) -> &'a str {
    value.pointer(pointer).and_then(Value::as_str).unwrap()
}

fn array<'a>(value: &'a Value, pointer: &str) -> &'a [Value] {
    value.pointer(pointer).and_then(Value::as_array).unwrap()
}

fn repository_delta(record: &AuthorityRecordV1) -> &ObjectDeltaV1 {
    record
        .content
        .object_delta
        .iter()
        .find(|delta| delta.path == ".vela/repository.json")
        .unwrap()
}

#[derive(Serialize)]
struct WriteSet<'a> {
    schema: &'static str,
    transaction_id: &'a str,
    before_event_log_root: &'a str,
    after_event_log_root: &'a str,
    event_ids: &'a [String],
    object_delta: &'a [ObjectDeltaV1],
}

fn write_set_root(record: &AuthorityRecordV1) -> String {
    let content = &record.content;
    let value = WriteSet {
        schema: "vela.authority-write-set.internal.v1",
        transaction_id: &content.transaction_id,
        before_event_log_root: &content.before_event_log_root,
        after_event_log_root: &content.after_event_log_root,
        event_ids: &content.event_ids,
        object_delta: &content.object_delta,
    };
    let mut digest = Sha256::new();
    digest.update(b"vela.authority-write-set.internal.v1\0");
    digest.update(to_canonical_bytes(&value).unwrap());
    format!("sha256:{:x}", digest.finalize())
}

fn manifest(root: &str) -> RepositoryV4 {
    let digest = root.strip_prefix("sha256:").unwrap();
    RepositoryV4::parse(&read(&format!("repository-manifests/{digest}.json"))).unwrap()
}

/// Trust-anchor selection is deliberately external to history verification.
fn anchor_selects(anchor: &TrustAnchor, verified: &AuthorityHistoryVerification) -> bool {
    anchor.repository_id == verified.repository_id
        && verified.first_authority_record_root.as_deref()
            == Some(anchor.first_authority_record_root.as_str())
}

#[test]
fn retained_math_authority_chain_verifies_from_an_external_anchor() {
    let fixture = Fixture::load();
    let expected = &fixture.expected;
    assert_eq!(fixture.anchor.schema, "vela.authority-trust-anchor.v1");
    let history = &fixture.history;
    assert_eq!(
        history.keysets[0].root().unwrap(),
        text(expected, "/authority_keyset_root")
    );
    assert_eq!(
        history.models[0].root().unwrap(),
        text(expected, "/authorization_model_root")
    );
    assert_eq!(
        format!("sha256:{}", sha256_canonical(&fixture.anchor).unwrap()),
        text(expected, "/trust_anchor_root")
    );

    let verified = fixture.verify();
    assert_eq!(verified.era, AuthorityHistoryEra::RepositoryAuthority);
    assert_eq!(
        (
            verified.authority_record_count,
            verified.authority_event_count
        ),
        (4, 7)
    );
    assert_eq!(
        verified.final_event_log_root,
        text(expected, "/final_event_log_root")
    );
    assert_eq!(
        verified.final_authority_record_root.as_deref(),
        Some(text(expected, "/final_authority_record_root"))
    );
    assert_eq!(
        verified.final_authority_keyset_root.as_deref(),
        Some(text(expected, "/authority_keyset_root"))
    );
    assert_eq!(
        verified.final_authorization_model_root.as_deref(),
        Some(text(expected, "/authorization_model_root"))
    );
    assert!(anchor_selects(&fixture.anchor, &verified));

    for (record, item) in fixture.records.iter().zip(array(expected, "/records")) {
        assert_eq!(record.content.sequence, item["sequence"].as_u64().unwrap());
        assert_eq!(record.root().unwrap(), text(item, "/record_root"));
        let content = &record.content;
        let request = &content.authorization.request;
        assert_eq!(
            write_set_root(record),
            content.execution.transaction_write_set_root
        );
        assert_eq!(
            serde_json::to_value(request.action).unwrap(),
            item["action"]
        );
        assert_eq!(
            content.authentication.root().unwrap(),
            request.authentication_root
        );
        assert_eq!(request.intent_digest, content.intent_digest);
        let delta = repository_delta(record);
        assert_ne!(
            request.transaction_read_set_root,
            content.execution.transaction_read_set_root
        );
        assert_eq!(
            request.transaction_read_set_root,
            delta.before_root.as_deref().unwrap_or(EMPTY_READ_ROOT)
        );
        let after = delta.after_root.as_deref().unwrap();
        assert_eq!(manifest(after).canonical_root().unwrap(), after);
    }

    let terminal_root = text(expected, "/terminal/repository_manifest_root");
    let final_delta = repository_delta(fixture.records.last().unwrap());
    assert_eq!(final_delta.after_root.as_deref(), Some(terminal_root));
    let terminal = manifest(terminal_root);
    let origin = RepositoryOriginV1::parse(&read("origin.json")).unwrap();
    // Terminal manifest/Claim binding is fixture-local, above the production verifier.
    assert_eq!(
        (
            terminal.accepted_claims.len(),
            terminal.pending_claims.len(),
            terminal.proposals.len(),
            terminal.proposal_withdrawals.len(),
            terminal.submissions.len(),
            terminal.verifications.len(),
            terminal.artifacts.len()
        ),
        (2, 0, 3, 0, 3, 3, 2)
    );
    for (accepted, item) in terminal
        .accepted_claims
        .iter()
        .zip(array(expected, "/terminal/accepted_claims"))
    {
        assert_eq!(accepted.claim_id, text(item, "/claim_id"));
        assert_eq!(accepted.claim_root, text(item, "/claim_root"));
    }
    assert_eq!(terminal.repository_id, text(expected, "/repository_id"));
    assert_eq!(terminal.origin_root, origin.canonical_root().unwrap());

    let mut previous_claim_id = None;
    for item in array(expected, "/terminal/applied_transitions") {
        let sequence = item["sequence"].as_u64().unwrap() as usize;
        let record = &array(expected, "/records")[sequence - 1];
        let applied = history
            .events
            .iter()
            .find(|event| event.id == text(item, "/event_id"))
            .unwrap();
        assert_eq!(
            serde_json::to_value(&applied.content.kind).unwrap(),
            item["kind"]
        );
        assert_eq!(applied.content.after_hash, text(item, "/claim_root"));
        assert_eq!(
            applied.content.target.id,
            if text(item, "/kind") == "claim.superseded" {
                previous_claim_id.unwrap()
            } else {
                text(item, "/claim_id")
            }
        );
        assert_eq!(applied.content.payload["claim_id"], item["claim_id"]);
        assert_eq!(applied.content.payload["claim_root"], item["claim_root"]);
        assert_eq!(
            applied.semantic_event_id().unwrap(),
            text(item, "/semantic_event_id")
        );
        let review = array(record, "/event_ids")
            .iter()
            .filter_map(Value::as_str)
            .map(|event_id| {
                history
                    .events
                    .iter()
                    .find(|event| event.id == event_id)
                    .unwrap()
            })
            .find(|event| serde_json::to_value(&event.content.kind).unwrap() == "review.accepted")
            .unwrap();
        assert_eq!(
            review.content.payload["applied_event_id"],
            item["semantic_event_id"]
        );
        previous_claim_id = Some(text(item, "/claim_id"));
    }
}

#[test]
fn retained_math_authority_falsifiers_fail_closed_without_resigning() {
    let fixture = Fixture::load();
    let mut anchor = fixture.anchor.clone();
    anchor.first_authority_record_root = format!("sha256:{}", "0".repeat(64));
    assert!(!anchor_selects(&anchor, &fixture.verify()));

    let mut history = fixture.history.clone();
    let signature = &mut history.envelopes[3].signatures[0].sig;
    signature.replace_range(..1, if signature.starts_with('A') { "B" } else { "A" });
    assert!(fixture.verify_with(&history).is_err());
    history = fixture.history.clone();
    history.envelopes.remove(1);
    assert!(fixture.verify_with(&history).is_err());

    history = fixture.history.clone();
    history.keysets[0].keys[0].valid_through_sequence = Some(1);
    assert!(fixture.verify_with(&history).is_err());
    history = fixture.history.clone();
    history.models[0].members.remove(1);
    assert!(fixture.verify_with(&history).is_err());

    history = fixture.history.clone();
    history.events.pop();
    assert!(fixture.verify_with(&history).is_err());
    history = fixture.history.clone();
    history
        .events
        .iter_mut()
        .find(|event| event.id == "vev_cd938718b5750d22")
        .unwrap()
        .content
        .payload["verdict"] = Value::String("rejected".into());
    assert!(fixture.verify_with(&history).is_err());

    let terminal_root = text(&fixture.expected, "/terminal/repository_manifest_root");
    let mut terminal = manifest(terminal_root);
    terminal.accepted_claims[1].claim_root = format!("sha256:{}", "0".repeat(64));
    let expected_root = text(&fixture.expected, "/terminal/accepted_claims/1/claim_root");
    assert_ne!(terminal.accepted_claims[1].claim_root, expected_root);
}
