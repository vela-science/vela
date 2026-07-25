use vela_protocol::sources::source_record_for_finding;
use vela_protocol::test_support::make_finding;

#[test]
fn source_commitment_is_stable_with_the_cli_feature_graph() {
    let mut finding = make_finding("vf_test", 0.6, "mechanism");
    finding.provenance.source_type = "model_output".to_string();
    finding.provenance.doi = Some("10.0000/test".to_string());
    finding.provenance.title = "cap_demo - claim_001".to_string();
    finding.provenance.url = Some("artifact_packet:cap_demo".to_string());
    finding.provenance.extraction.method = "artifact_to_state_import".to_string();
    finding.provenance.extraction.extracted_at = "2026-05-06T00:00:00Z".to_string();
    finding.provenance.extraction.extractor_version = "vela/0.55.0".to_string();

    let record = source_record_for_finding(&finding);

    assert_eq!(record.source_type, "synthetic_report");
    assert_eq!(
        record.content_hash.as_deref(),
        Some("sha256:e19b1285f2e3757335079ff945e62f387b6edfa08b517b99402e9425eed97987")
    );
    assert_eq!(record.id, "vs_cf51972d2d6fe9d7");
}
