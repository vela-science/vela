use serde_json::Value;
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("../../../conformance/fixtures/exact-witness-floor-v1.json");

#[test]
fn rust_reference_rederives_the_exact_witness_floor_fixture() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(fixture["schema"], "vela.exact-witness-floor-fixture.v1");
    assert_eq!(fixture["artifact_kind"], "vela-witness");
    assert_eq!(fixture["replayability"], "exact");

    let witness: vela_verify::Witness =
        serde_json::from_value(fixture["witness"].clone()).expect("witness parses");
    let canonical = vela_protocol::canonical::to_canonical_bytes(&fixture["witness"])
        .expect("witness canonicalizes");
    assert_eq!(
        fixture["witness_sha256"],
        format!("sha256:{:x}", Sha256::digest(canonical))
    );
    assert!(vela_verify::verify_witness(&witness).ok);

    for case in fixture["claims"].as_array().expect("claims are an array") {
        let actual = vela_verify::claim_witness_faithful(
            case["text"].as_str().expect("claim text is a string"),
            &witness,
        )
        .faithful;
        assert_eq!(actual, case["faithful"], "{}", case["id"]);
    }

    let corrupted: vela_verify::Witness =
        serde_json::from_value(fixture["corrupted_witness"].clone())
            .expect("corrupted witness still parses");
    assert!(!vela_verify::verify_witness(&corrupted).ok);
}
