use std::fs;
use std::path::PathBuf;

use vela_protocol::detached::{DetachedSignature, verify_detached};

const MANIFEST_NAME: &str = "fixtures.manifest.json";

#[test]
fn active_fixture_manifest_signature_is_absent_or_exact() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures = root.join("conformance/fixtures");
    let signature_path = fixtures.join("fixtures.manifest.sig.json");
    if !signature_path.exists() {
        eprintln!("fixtures manifest is explicitly unsigned; human ceremony remains open");
        return;
    }

    let record: DetachedSignature = serde_json::from_slice(
        &fs::read(&signature_path).expect("read fixtures manifest signature"),
    )
    .expect("parse canonical detached signature");
    assert_eq!(
        record.subject, MANIFEST_NAME,
        "detached signature must name the fixtures manifest"
    );
    let manifest = fs::read(fixtures.join(MANIFEST_NAME)).expect("read fixtures manifest");
    verify_detached(&manifest, &record)
        .expect("active fixtures manifest signature must bind the exact current bytes");
}
