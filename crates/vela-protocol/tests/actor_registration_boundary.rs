use serde_json::Value;

#[test]
fn actor_registration_boundary_erdos_regression_vector_is_exact() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/actor-registration-boundary-v1.json"
    ))
    .unwrap();
    let erdos = &fixture["erdos_regression"];
    assert_eq!(
        erdos["registration_commit"],
        "bba62e8d85393887f00caffe7c28d005a1786b3f"
    );
    assert_eq!(
        erdos["anchor_tree"],
        "2c5c5a6c688a274b40017321d920f258f1c70c04"
    );
    assert_eq!(erdos["anchor_event_count"], 2185);
    assert_eq!(erdos["actor_events"]["unsigned_anchor"], 81);
    assert_eq!(erdos["actor_events"]["signed_anchor"], 131);
    assert_eq!(erdos["actor_events"]["signed_post_anchor"], 1);
    assert_eq!(erdos["actor_events"]["signed_total"], 132);
    assert_eq!(erdos["post_anchor_event_id"], "vev_27922b9c8dab0575");
    assert_eq!(
        erdos["expected_after_activation"]["anchored_event_file_delta"],
        0
    );
}

#[test]
fn actor_registration_boundary_erdos_preview_matches_registered_vector() {
    let registered: Value = serde_json::from_str(include_str!(
        "../../../conformance/actor-registration-boundary-v1.json"
    ))
    .unwrap();
    let preview: Value = serde_json::from_str(include_str!(
        "../../../conformance/erdos-actor-registration-preview-v1.json"
    ))
    .unwrap();
    let erdos = &registered["erdos_regression"];
    let payload = &preview["preview"]["payload"];
    assert_eq!(
        payload["anchor"]["git_commit"],
        erdos["registration_commit"]
    );
    assert_eq!(payload["anchor"]["git_tree"], erdos["anchor_tree"]);
    assert_eq!(
        payload["anchor"]["event_log_root"],
        erdos["anchor_event_log_root"]
    );
    assert_eq!(
        payload["anchor"]["actor_registry_root"],
        erdos["anchor_actor_registry_root"]
    );
    assert_eq!(
        preview["preview"]["counts"]["anchored_unsigned"],
        erdos["actor_events"]["unsigned_anchor"]
    );
    assert_eq!(
        preview["preview"]["counts"]["post_anchor_signed"],
        erdos["actor_events"]["signed_post_anchor"]
    );
    assert_eq!(preview["candidate"]["key_read"], false);
    assert_eq!(preview["candidate"]["frontier_mutation"], false);
}
