//! `vela.status.v4`: the one document `vela status` answers with.
//!
//! This was two `serde_json::json!` literals in `vela-cli`, one per branch —
//! a repository whose repository authority has not finished initializing, and a
//! replaying one. They were meant to be the same document and they were not:
//! the first spelled its own `schema` field `vela.status.v1` for the whole
//! life of v2 and v3, so a caller keying on `schema` met a version it had no
//! reader for and could not tell a cold repository from a stale release. One
//! type is what makes that unwritable rather than merely wrong.
//!
//! The second reader is out of this tree. `vela-web` parses this document into
//! its Observatory projection, and in six days it took three commits to follow
//! shape changes made here: `counts.withdrawn_review` arriving, `git.role`
//! arriving, and `actions.work.mode` moving from one value to a two-member
//! union. Each landed as a fail-closed break of the projection refresh, found
//! by running it rather than by anything holding the two shapes together.
//! `wire_schema::published()` renders this type to
//! `schemas/status.schema.json`, and `tests/wire_schemas.rs` holds the file
//! to the type byte for byte. That seals one link and not the next one.
//! `vela-web` contains no reference to `status.schema.json` anywhere in its
//! tree, so its parser is still held to this type by nothing but running the
//! refresh and watching it fail. The published schema is what that consumer
//! could gate on. This comment used to say it was what the consumer does gate
//! on, which was the same error, one level up, as the three breakages above.
//!
//! ## Null is not absence here
//!
//! Every optional-looking field on this document is *present and null* on the
//! branch that cannot fill it — a bootstrapping repository has a Git pointer
//! with the role `repository_head` and no commit behind it yet, not an absent
//! pointer. The `json!` literals spelled that as an explicit `Value::Null`.
//!
//! `Option<T>` alone does not say that. serde still emits the key, but
//! schemars leaves a bare `Option` field out of `required`, so the published
//! schema would accept a document with the key dropped, and the consumer
//! gating against it would stop noticing. Every such field therefore carries
//! `#[schemars(required, schema_with = ...)]` — the builder supplies the null
//! arm of the type, and `required` says the key is not optional.
//!
//! Neither attribute is trusted to be the one that works.
//! `every_field_is_required_on_the_wire` walks the rendered document and holds
//! every property of every object to its own `required` list, which is the
//! statement that actually matters and the only one that survives a schemars
//! upgrade changing what these attributes imply.
//!
//! ## An extra field is not absence either
//!
//! Eleven objects here carried `#[serde(deny_unknown_fields)]` until
//! 2026-08-07, which put `"additionalProperties": false` on every one of them
//! in the published schema. Read against the paragraph above, that combination
//! detects nothing the `required` list does not already detect. A dropped or
//! renamed field fails because the field it replaced is required. Closure
//! catches only the other direction — a field this version does not name — and
//! on a document that roots nothing, signs nothing, and is derived from state
//! that already exists, an extra field is not evidence of anything. It is this
//! document with more in it.
//!
//! What the closure did instead is the three breakages at the top of this file.
//! All three were additive. It converted three free changes into three
//! fail-closed breaks of a downstream refresh and made zero detections in the
//! same six days.
//!
//! So this document is open, and its consumers are obliged to ignore what they
//! do not recognize. `docs/INTEROPERABILITY.md` states the rule and its
//! opposite together, because the opposite is the one that matters more: a
//! signed preimage has no compatible change at all. `rooted()` hashes canonical
//! JSON *including keys*, so a field added to a signed object is a different
//! object with a different root, `deny_unknown_fields` on those types is the
//! enforcement, and a parser that shrugs at an unknown field there is not
//! lenient — it is non-conformant. Nothing in this module's relaxation reaches
//! across that line, and nothing should be carried back across it either.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The schema tag this document carries. `vela-web` pins it as a zod literal.
pub const STATUS_V4_SCHEMA: &str = "vela.status.v4";

/// The `command` tag this document carries, matching the verb that emits it.
pub const STATUS_V4_COMMAND: &str = "status";

/// The one role a repository's tracked Git pointer plays.
///
/// It is what the pointer *means*, not whether it has reached a commit, which
/// is why it is stated even on the branch where `commit` and `tree` are null.
pub const REPOSITORY_HEAD_ROLE: &str = "repository_head";

/// Whether replay reproduced the retained history.
///
/// `verified` is a wire token, not prose. The CLI contract forbids the
/// unqualified word in text; consumers compare this value, so retiring it
/// requires a coordinated schema change rather than a wording change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReplayState {
    Verified,
    NotInitialized,
}

/// Whether strict verification passed, or is blocked by a stated code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StrictState {
    Pass,
    Blocked,
}

/// The repository this document is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusRepository {
    #[schemars(schema_with = "crate::wire_schema::repository_id")]
    pub id: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub name: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub profile_root: String,
}

/// The Git pointer the repository's bytes are published under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusGit {
    #[schemars(schema_with = "crate::wire_schema::repository_head_role_tag")]
    pub role: String,
    #[schemars(required, schema_with = "crate::wire_schema::nullable_git_object_id")]
    pub commit: Option<String>,
    #[schemars(required, schema_with = "crate::wire_schema::nullable_git_object_id")]
    pub tree: Option<String>,
}

/// What replay and strict verification found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusIntegrity {
    pub replay: ReplayState,
    pub strict: StrictState,
    pub blocker_count: u64,
    /// Blocker codes to their occurrence counts. Empty on a passing repository;
    /// the map, not a separate list, is what a consumer sums.
    pub blockers_by_code: BTreeMap<String, u64>,
}

/// The four roots that identify the repository's current state.
///
/// All four are null together, on the branch where repository authority has
/// not finished initializing and there is no repository to root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusRoots {
    #[schemars(required, schema_with = "crate::wire_schema::nullable_sha256_root")]
    pub origin: Option<String>,
    #[schemars(required, schema_with = "crate::wire_schema::nullable_sha256_root")]
    pub repository: Option<String>,
    #[schemars(required, schema_with = "crate::wire_schema::nullable_sha256_root")]
    pub authority_keyset: Option<String>,
    #[schemars(required, schema_with = "crate::wire_schema::nullable_sha256_root")]
    pub authority_policy: Option<String>,
}

/// What the repository holds, by object kind and standing.
///
/// `claims` partitions into `accepted_claims` and `pending_claims`; the
/// consumer checks that partition, so the three travel together.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusCounts {
    pub claims: u64,
    pub accepted_claims: u64,
    pub pending_claims: u64,
    pub pending_review: u64,
    pub accepted_review: u64,
    pub rejected_review: u64,
    pub withdrawn_review: u64,
    pub submissions: u64,
    pub verifications: u64,
    pub artifacts: u64,
}

/// The Decision Inbox summary, rooted so a consumer can bind to the exact
/// projection this count was taken over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusDecisionInbox {
    pub pending_count: u64,
    pub protocol_ready_count: u64,
    pub protocol_blocked_count: u64,
    #[schemars(required, schema_with = "crate::wire_schema::nullable_sha256_root")]
    pub projection_root: Option<String>,
    #[schemars(required, schema_with = "crate::wire_schema::nullable_sha256_root")]
    pub first_entry_root: Option<String>,
}

/// The one review action, when a Decision is waiting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusReviewAction {
    pub pending_count: u64,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub command: String,
}

/// The one producer action, and what determines which one it is.
///
/// The mode is the discriminant a consumer switches on, and `note` is present
/// on exactly the two modes that need to explain themselves. Spelling this as
/// a tagged enum is what keeps `note` from becoming an always-present field
/// that is empty two thirds of the time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum StatusWorkAction {
    /// Bounded evidence is submitted directly.
    DirectSubmission {
        #[schemars(schema_with = "crate::wire_schema::text")]
        command: String,
        #[schemars(schema_with = "crate::wire_schema::text")]
        note: String,
    },
    /// Repository authority has not finished initializing. Nothing can
    /// produce, verify, or decide until `vela init` completes, so this mode
    /// names that one command and no work is offered.
    AuthorityUninitialized {
        #[schemars(schema_with = "crate::wire_schema::text")]
        command: String,
        #[schemars(schema_with = "crate::wire_schema::text")]
        note: String,
    },
}

impl StatusWorkAction {
    /// The one command this mode offers. Every mode has exactly one, which is
    /// the point of the lane, so the human-readable renderer reads it here
    /// rather than matching three times.
    pub fn command(&self) -> &str {
        match self {
            Self::DirectSubmission { command, .. }
            | Self::AuthorityUninitialized { command, .. } => command,
        }
    }
}

/// The two independent lanes: what a reviewer may do, and what a producer may
/// do. They were one scalar `next_action` through v2, which forced a repository
/// with both to name only one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusActions {
    #[schemars(required, schema_with = "crate::wire_schema::nullable_review_action")]
    pub review: Option<StatusReviewAction>,
    pub work: StatusWorkAction,
}

/// The whole document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusV4 {
    #[schemars(schema_with = "crate::wire_schema::status_schema_tag")]
    pub schema: String,
    #[schemars(schema_with = "crate::wire_schema::ok_true")]
    pub ok: bool,
    #[schemars(schema_with = "crate::wire_schema::status_command_tag")]
    pub command: String,
    pub repository: StatusRepository,
    pub git: StatusGit,
    pub integrity: StatusIntegrity,
    pub roots: StatusRoots,
    pub counts: StatusCounts,
    pub decision_inbox: StatusDecisionInbox,
    pub actions: StatusActions,
}

impl StatusV4 {
    /// Build the envelope every branch shares, so the three tags cannot be
    /// spelled per-branch again.
    pub fn new(
        repository: StatusRepository,
        git: StatusGit,
        integrity: StatusIntegrity,
        roots: StatusRoots,
        counts: StatusCounts,
        decision_inbox: StatusDecisionInbox,
        actions: StatusActions,
    ) -> Self {
        Self {
            schema: STATUS_V4_SCHEMA.into(),
            ok: true,
            command: STATUS_V4_COMMAND.into(),
            repository,
            git,
            integrity,
            roots,
            counts,
            decision_inbox,
            actions,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    /// Nothing on this document is optional on the wire.
    ///
    /// A field this crate models as `Option` is a field that is present and
    /// null, never a field that is absent — see the module header. schemars
    /// leaves `Option` out of `required` by default, so without this the
    /// published schema would silently permit an absent `git.commit` and the
    /// consumer that gates against it would stop noticing a dropped key.
    ///
    /// Proven to fail: removing `required` from `StatusGit::commit` alone
    /// makes this report `properties.git: required is missing commit`.
    #[test]
    fn every_field_is_required_on_the_wire() {
        fn walk(node: &Value, path: &str, gaps: &mut Vec<String>) {
            let Value::Object(map) = node else { return };
            if let (Some(Value::Object(properties)), required) =
                (map.get("properties"), map.get("required"))
            {
                let declared: Vec<&str> = required
                    .and_then(Value::as_array)
                    .map(|items| items.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                for name in properties.keys() {
                    if !declared.contains(&name.as_str()) {
                        gaps.push(format!("{path}: required is missing {name}"));
                    }
                }
            }
            for (key, value) in map {
                match value {
                    Value::Object(_) => walk(value, &format!("{path}.{key}"), gaps),
                    Value::Array(items) => {
                        for (index, item) in items.iter().enumerate() {
                            walk(item, &format!("{path}.{key}[{index}]"), gaps);
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut gaps = Vec::new();
        walk(&published_schema(), "status", &mut gaps);
        assert!(gaps.is_empty(), "{gaps:#?}");
    }

    fn published_schema() -> Value {
        crate::wire_schema::published()
            .into_iter()
            .find(|(file, _)| *file == "status.schema.json")
            .expect("status v3 is published")
            .1
    }

    /// The document serde writes carries every key the schema requires.
    ///
    /// The schema and the serializer are generated from one type, so they
    /// agree by construction — until a field acquires
    /// `#[serde(skip_serializing_if = "Option::is_none")]`, which is the house
    /// style for the signed objects next door and would be a reasonable-looking
    /// edit here. That one attribute would silently drop `git.commit` from a
    /// bootstrapping repository's status while the schema went on requiring it,
    /// and the consumer would read a dropped field as an absent one.
    ///
    /// The subject is the emptiest document the type can hold, because that is
    /// the one where every nullable field is exercised at once.
    ///
    /// Proven to fail: adding that attribute to `StatusGit::commit` makes this
    /// report `git: serialized without commit`.
    #[test]
    fn the_serialized_document_carries_every_required_key() {
        let bootstrapping = super::StatusV4::new(
            super::StatusRepository {
                id: "00000000-0000-4000-8000-000000000000".into(),
                name: "fixture".into(),
                profile_root: format!("sha256:{}", "0".repeat(64)),
            },
            super::StatusGit {
                role: super::REPOSITORY_HEAD_ROLE.into(),
                commit: None,
                tree: None,
            },
            super::StatusIntegrity {
                replay: super::ReplayState::NotInitialized,
                strict: super::StrictState::Blocked,
                blocker_count: 1,
                blockers_by_code: super::BTreeMap::new(),
            },
            super::StatusRoots {
                origin: None,
                repository: None,
                authority_keyset: None,
                authority_policy: None,
            },
            super::StatusCounts::default(),
            super::StatusDecisionInbox {
                pending_count: 0,
                protocol_ready_count: 0,
                protocol_blocked_count: 0,
                projection_root: None,
                first_entry_root: None,
            },
            super::StatusActions {
                review: None,
                work: super::StatusWorkAction::AuthorityUninitialized {
                    command: "vela init . --json".into(),
                    note: "fixture".into(),
                },
            },
        );
        let document = serde_json::to_value(&bootstrapping).expect("status serializes");

        // Walk the schema and the document together, so only the objects the
        // document actually reaches are checked — the `work` action is a
        // three-member union and one document is one member of it.
        fn walk(schema: &Value, value: &Value, path: &str, gaps: &mut Vec<String>) {
            let (Some(properties), Some(object)) = (schema.get("properties"), value.as_object())
            else {
                return;
            };
            for name in schema
                .get("required")
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Value::as_str))
                .into_iter()
                .flatten()
            {
                if !object.contains_key(name) {
                    gaps.push(format!("{path}: serialized without {name}"));
                }
            }
            for (name, child) in object {
                if let Some(subschema) = properties.get(name) {
                    walk(subschema, child, &format!("{path}.{name}"), gaps);
                }
            }
        }

        let mut gaps = Vec::new();
        walk(&published_schema(), &document, "status", &mut gaps);
        assert!(gaps.is_empty(), "{gaps:#?}");
    }
}
