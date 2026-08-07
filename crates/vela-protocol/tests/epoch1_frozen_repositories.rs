//! The epoch-1 reader is checked against the repositories it exists for.
//!
//! A frozen reader that is never run against frozen bytes is a guess. This
//! parses each retained checkout with the epoch-1 types and rebuilds the roots
//! those repositories already declare, so a change to the current objects that
//! silently broke epoch-1 parsing would fail here rather than the first time
//! someone opened an old repository.
//!
//! `#[ignore]`d and env-driven for the same reason as the JCS shadow audit: it
//! needs local checkouts that are not part of this repository.

use std::env;
use std::fs;
use std::path::Path;

use vela_protocol::epoch1::{
    is_epoch1_scientific_event_kind, Epoch1OriginV1, Epoch1ProfileV2, Epoch1RepositoryV4,
    EPOCH1_PROFILE_PATH,
};

fn read(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
#[ignore = "requires the local epoch-1 Frontier checkouts"]
fn epoch1_repositories_parse_and_reproduce_their_roots() {
    let roots = env::var_os("VELA_EPOCH1_ROOTS")
        .map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .filter(|paths| !paths.is_empty())
        .expect("VELA_EPOCH1_ROOTS must contain the epoch-1 repository paths");

    for checkout in roots {
        let label = checkout.display().to_string();

        let profile = Epoch1ProfileV2::from_toml_str(
            &String::from_utf8(read(&checkout.join(EPOCH1_PROFILE_PATH)))
                .unwrap_or_else(|error| panic!("{label}: {EPOCH1_PROFILE_PATH} is not UTF-8: {error}")),
        )
        .unwrap_or_else(|error| panic!("{label}: profile did not parse: {error}"));

        let origin = Epoch1OriginV1::parse(&read(&checkout.join(".vela/origin.json")))
            .unwrap_or_else(|error| panic!("{label}: origin did not parse: {error}"));

        let repository = Epoch1RepositoryV4::parse(&read(&checkout.join(".vela/repository.json")))
            .unwrap_or_else(|error| panic!("{label}: repository did not parse: {error}"));

        // The three identities agree, and the profile root the origin declares is
        // the one this reader rebuilds from the profile bytes.
        assert_eq!(profile.frontier_id, origin.frontier_id, "{label}: profile and origin disagree on frontier_id");
        assert_eq!(repository.frontier_id, origin.frontier_id, "{label}: repository and origin disagree on frontier_id");
        assert_eq!(repository.origin_id, origin.origin_id, "{label}: repository and origin disagree on origin_id");
        assert_eq!(
            profile.profile_root().expect("profile root"),
            origin.profile_root,
            "{label}: rebuilt profile root does not match the one the origin declares"
        );
        assert_eq!(
            origin.canonical_root().expect("origin root"),
            repository.origin_root,
            "{label}: rebuilt origin root does not match the one the repository declares"
        );

        // Parsing already re-serialized and compared bytes, so a successful
        // canonical_root here means the retained file is still canonical.
        repository
            .canonical_root()
            .unwrap_or_else(|error| panic!("{label}: repository root did not rebuild: {error}"));

        // Every retained Event kind is still recognised as epoch 1 spelled it.
        // This is the exact check whose current-path equivalent stopped matching
        // when the kinds were renamed.
        let events = checkout.join(".vela/authority/events");
        let mut scientific = 0usize;
        for entry in fs::read_dir(&events).unwrap_or_else(|error| panic!("{label}: {error}")) {
            let path = entry.expect("dir entry").path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let event: serde_json::Value = serde_json::from_slice(&read(&path))
                .unwrap_or_else(|error| panic!("{label}: {} did not parse: {error}", path.display()));
            let kind = event["content"]["kind"].as_str().unwrap_or_else(|| {
                panic!("{label}: {} has no content.kind", path.display())
            });
            if is_epoch1_scientific_event_kind(kind) {
                scientific += 1;
            }
        }

        println!("{label}: {} accepted claims, {scientific} scientific events", repository.accepted_claims.len());
    }
}
