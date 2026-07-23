//! Pure protocol values for Frontier repository identity and boundary events.
//!
//! This module deliberately does not inspect Git, resolve dependency locators,
//! enumerate files, or decide actor authority. It validates the closed wire
//! values, recomputes the roots available from those values, verifies ordinary
//! event shape/signatures, and checks continuity against a supplied previous
//! boundary. A caller must still verify the anchored Git objects, retained
//! bytes, and actor-registry state before treating a boundary as valid.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::actor_registration::{require_lower_hex, require_sha256_root};
use crate::events::{
    EVENT_KIND_FRONTIER_REPOSITORY_BOUND, EVENT_SCHEMA, FRONTIER_CREATED_SCHEMA_V1, NULL_HASH,
    StateActor, StateEvent, StateTarget, compute_event_id, event_content_preimage_bytes,
};

pub const FRONTIER_IDENTITY_SCHEMA: &str = "vela.frontier-identity.v1";
pub const LEGACY_FRONTIER_ORIGIN_SCHEMA: &str = "vela.legacy-frontier-origin.v1";
pub const FRONTIER_REPOSITORY_BOUNDARY_SCHEMA: &str = "vela.frontier-repository-boundary.v1";
pub const RETAINED_OBJECT_MANIFEST_SCHEMA: &str = "vela.retained-object-manifest.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontierIdentityOrigin {
    Genesis,
    LegacyBoundary,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GitObjectFormat {
    Sha1,
    Sha256,
}

impl GitObjectFormat {
    fn digest_len(self) -> usize {
        match self {
            Self::Sha1 => 40,
            Self::Sha256 => 64,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrontierIdentityV1 {
    pub schema: String,
    pub frontier_id: String,
    pub origin: FrontierIdentityOrigin,
    pub origin_commitment: String,
    pub legacy_identity_preimage_root: Option<String>,
}

impl FrontierIdentityV1 {
    /// Derive, rather than accept, the identity of a new Frontier from its
    /// exact `frontier.created` event preimage.
    pub fn from_genesis_event(event: &StateEvent) -> Result<Self, String> {
        validate_profile_v1_genesis_event(event)?;
        let digest = hex::encode(Sha256::digest(event_content_preimage_bytes(event)));
        Ok(Self {
            schema: FRONTIER_IDENTITY_SCHEMA.to_string(),
            frontier_id: format!("vfr_{}", &digest[..16]),
            origin: FrontierIdentityOrigin::Genesis,
            origin_commitment: format!("sha256:{digest}"),
            legacy_identity_preimage_root: None,
        })
    }

    /// Validate the closed record shape only.
    ///
    /// For `origin: genesis`, this does not prove that `origin_commitment`
    /// came from a real `frontier.created` event. Callers establishing
    /// repository identity must derive the record with
    /// [`Self::from_genesis_event`] or validate a boundary chain.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FRONTIER_IDENTITY_SCHEMA {
            return Err(format!(
                "identity.schema must be {FRONTIER_IDENTITY_SCHEMA}"
            ));
        }
        require_frontier_id("identity.frontier_id", &self.frontier_id)?;
        require_sha256_root("identity.origin_commitment", &self.origin_commitment)?;
        match (self.origin, self.legacy_identity_preimage_root.as_deref()) {
            (FrontierIdentityOrigin::Genesis, None) => Ok(()),
            (FrontierIdentityOrigin::Genesis, Some(_)) => {
                Err("genesis identity must use a null legacy_identity_preimage_root".to_string())
            }
            (FrontierIdentityOrigin::LegacyBoundary, Some(root)) => {
                require_sha256_root("identity.legacy_identity_preimage_root", root)
            }
            (FrontierIdentityOrigin::LegacyBoundary, None) => {
                Err("legacy_boundary identity requires legacy_identity_preimage_root".to_string())
            }
        }
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        canonical_root(self)
    }

    pub fn verify_root(&self, expected: &str) -> Result<(), String> {
        require_sha256_root("identity_root", expected)?;
        if self.root()? != expected {
            return Err("identity_root does not match the closed identity preimage".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LegacyFrontierOriginV1 {
    pub schema: String,
    pub frontier_id: String,
    pub legacy_identity_preimage_root: String,
    pub git_object_format: GitObjectFormat,
    pub anchor_git_commit: String,
    pub anchor_git_tree: String,
    pub anchor_event_log_root: String,
    pub anchor_event_count: u64,
}

impl LegacyFrontierOriginV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != LEGACY_FRONTIER_ORIGIN_SCHEMA {
            return Err(format!(
                "legacy origin schema must be {LEGACY_FRONTIER_ORIGIN_SCHEMA}"
            ));
        }
        require_frontier_id("legacy_origin.frontier_id", &self.frontier_id)?;
        require_sha256_root(
            "legacy_origin.legacy_identity_preimage_root",
            &self.legacy_identity_preimage_root,
        )?;
        require_git_object(
            "legacy_origin.anchor_git_commit",
            &self.anchor_git_commit,
            self.git_object_format,
        )?;
        require_git_object(
            "legacy_origin.anchor_git_tree",
            &self.anchor_git_tree,
            self.git_object_format,
        )?;
        require_sha256_root(
            "legacy_origin.anchor_event_log_root",
            &self.anchor_event_log_root,
        )?;
        if self.anchor_event_count == 0 {
            return Err("legacy origin requires a non-empty anchored event log".to_string());
        }
        Ok(())
    }

    pub fn origin_commitment(&self) -> Result<String, String> {
        self.validate()?;
        canonical_root(self)
    }

    pub fn identity(&self) -> Result<FrontierIdentityV1, String> {
        Ok(FrontierIdentityV1 {
            schema: FRONTIER_IDENTITY_SCHEMA.to_string(),
            frontier_id: self.frontier_id.clone(),
            origin: FrontierIdentityOrigin::LegacyBoundary,
            origin_commitment: self.origin_commitment()?,
            legacy_identity_preimage_root: Some(self.legacy_identity_preimage_root.clone()),
        })
    }

    pub fn identity_root(&self) -> Result<String, String> {
        self.identity()?.root()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExactFrontierDependencyV1 {
    pub frontier_id: String,
    pub identity_root: String,
    pub scientific_state_root: String,
    pub git_object_format: GitObjectFormat,
    pub git_commit: String,
    pub git_tree: String,
}

impl ExactFrontierDependencyV1 {
    pub fn validate(&self) -> Result<(), String> {
        require_frontier_id("dependency.frontier_id", &self.frontier_id)?;
        require_sha256_root("dependency.identity_root", &self.identity_root)?;
        require_sha256_root(
            "dependency.scientific_state_root",
            &self.scientific_state_root,
        )?;
        require_git_object(
            "dependency.git_commit",
            &self.git_commit,
            self.git_object_format,
        )?;
        require_git_object(
            "dependency.git_tree",
            &self.git_tree,
            self.git_object_format,
        )
    }

    fn canonical_key(&self) -> (&str, &str) {
        (&self.frontier_id, &self.identity_root)
    }
}

pub fn validate_exact_dependencies(
    dependencies: &[ExactFrontierDependencyV1],
) -> Result<(), String> {
    let mut previous: Option<(&str, &str)> = None;
    for dependency in dependencies {
        dependency.validate()?;
        let key = dependency.canonical_key();
        if let Some(prior) = previous {
            if key == prior {
                return Err(format!(
                    "duplicate dependency key ({}, {})",
                    dependency.frontier_id, dependency.identity_root
                ));
            }
            if key < prior {
                return Err(
                    "dependencies must be sorted by (frontier_id, identity_root)".to_string(),
                );
            }
        }
        previous = Some(key);
    }
    Ok(())
}

pub fn exact_dependency_root(dependencies: &[ExactFrontierDependencyV1]) -> Result<String, String> {
    validate_exact_dependencies(dependencies)?;
    canonical_root(dependencies)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetainedObjectEntryV1 {
    pub path: String,
    pub git_mode: String,
    pub size: u64,
    /// Lowercase, bare SHA-256 digest of the exact retained bytes.
    pub sha256: String,
}

impl RetainedObjectEntryV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_repository_path(&self.path)?;
        if !matches!(self.git_mode.as_str(), "100644" | "100755") {
            return Err(
                "retained object git_mode must be tracked regular-file mode 100644 or 100755"
                    .to_string(),
            );
        }
        require_lower_hex("retained object sha256", &self.sha256, 64)
    }
}

/// `vela.retained-object-manifest.v1` is represented on the wire as the exact
/// canonical JSON list, rather than an object wrapper. The schema constant is
/// the protocol label for that list and for conformance documentation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct RetainedObjectManifestV1(pub Vec<RetainedObjectEntryV1>);

impl RetainedObjectManifestV1 {
    pub fn validate(&self) -> Result<(), String> {
        let mut previous_path: Option<&str> = None;
        let mut portable_collision_keys = BTreeSet::new();
        for entry in &self.0 {
            entry.validate()?;
            if let Some(previous) = previous_path {
                if entry.path == previous {
                    return Err(format!("duplicate retained object path {:?}", entry.path));
                }
                if entry.path.as_str() < previous {
                    return Err("retained object entries must be sorted by path".to_string());
                }
            }
            previous_path = Some(&entry.path);

            // This is a deliberately conservative portable key, not an
            // emulation of any particular operating system: NFC, then Unicode
            // lowercase. It prevents a manifest from relying on case-sensitive
            // checkout behavior that common consumer platforms do not share.
            let collision_key = entry
                .path
                .nfc()
                .flat_map(char::to_lowercase)
                .collect::<String>();
            if !portable_collision_keys.insert(collision_key) {
                return Err(format!(
                    "retained object path {:?} has a portable case-fold collision",
                    entry.path
                ));
            }
        }
        Ok(())
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        canonical_root(&self.0)
    }

    pub fn verify_root(&self, expected: &str) -> Result<(), String> {
        require_sha256_root("retained_object_manifest_root", expected)?;
        if self.root()? != expected {
            return Err(
                "retained-object manifest root does not match its canonical entries".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontierRepositoryBoundaryMode {
    TemporalizeExisting,
    UpdateDependencies,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontierRepositoryTrustMode {
    Tofu,
    Genesis,
    PreviousBoundary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrontierRepositoryBoundaryPayloadV1 {
    pub schema: String,
    pub mode: FrontierRepositoryBoundaryMode,
    pub frontier_id: String,
    pub identity_root: String,
    pub observed_profile_root: String,
    pub dependency_root: String,
    pub dependencies: Vec<ExactFrontierDependencyV1>,
    /// Full content root of either the `frontier.created` genesis event (for
    /// the first dependency update of a new Frontier) or the preceding valid
    /// repository-boundary event.
    pub previous_identity_event_root: Option<String>,
    pub legacy_identity_preimage_root: Option<String>,
    pub administrator_actor_id: String,
    pub administrator_public_key: String,
    pub administrator_algorithm: String,
    pub trust_mode: FrontierRepositoryTrustMode,
    pub git_object_format: GitObjectFormat,
    pub anchor_git_commit: String,
    pub anchor_git_tree: String,
    pub anchor_event_log_root: String,
    pub anchor_event_count: u64,
    pub anchor_snapshot_root: String,
    pub anchor_snapshot_schema: String,
    pub anchor_proposal_root: String,
    pub anchor_actor_registry_root: String,
    pub anchor_artifact_registry_root: String,
    pub anchor_canonical_store_root: String,
}

impl FrontierRepositoryBoundaryPayloadV1 {
    /// Validate only properties derivable from this closed payload.
    ///
    /// `update_dependencies` identity continuity is necessarily checked by
    /// [`Self::validate_chain`], because the current payload intentionally does
    /// not repeat the original identity preimage.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FRONTIER_REPOSITORY_BOUNDARY_SCHEMA {
            return Err(format!(
                "payload.schema must be {FRONTIER_REPOSITORY_BOUNDARY_SCHEMA}"
            ));
        }
        require_frontier_id("payload.frontier_id", &self.frontier_id)?;
        for (field, root) in [
            ("payload.identity_root", self.identity_root.as_str()),
            (
                "payload.observed_profile_root",
                self.observed_profile_root.as_str(),
            ),
            ("payload.dependency_root", self.dependency_root.as_str()),
            (
                "payload.anchor_event_log_root",
                self.anchor_event_log_root.as_str(),
            ),
            (
                "payload.anchor_snapshot_root",
                self.anchor_snapshot_root.as_str(),
            ),
            (
                "payload.anchor_proposal_root",
                self.anchor_proposal_root.as_str(),
            ),
            (
                "payload.anchor_actor_registry_root",
                self.anchor_actor_registry_root.as_str(),
            ),
            (
                "payload.anchor_artifact_registry_root",
                self.anchor_artifact_registry_root.as_str(),
            ),
            (
                "payload.anchor_canonical_store_root",
                self.anchor_canonical_store_root.as_str(),
            ),
        ] {
            require_sha256_root(field, root)?;
        }
        if let Some(root) = self.previous_identity_event_root.as_deref() {
            require_sha256_root("payload.previous_identity_event_root", root)?;
        }
        if let Some(root) = self.legacy_identity_preimage_root.as_deref() {
            require_sha256_root("payload.legacy_identity_preimage_root", root)?;
        }
        require_git_object(
            "payload.anchor_git_commit",
            &self.anchor_git_commit,
            self.git_object_format,
        )?;
        require_git_object(
            "payload.anchor_git_tree",
            &self.anchor_git_tree,
            self.git_object_format,
        )?;
        if self.anchor_event_count == 0 {
            return Err("repository boundary requires a non-empty anchored event log".to_string());
        }
        require_text(
            "payload.anchor_snapshot_schema",
            &self.anchor_snapshot_schema,
        )?;
        if !(self.administrator_actor_id.starts_with("reviewer:")
            || self.administrator_actor_id.starts_with("steward:"))
        {
            return Err(
                "payload.administrator_actor_id must identify a reviewer: or steward: human actor"
                    .to_string(),
            );
        }
        require_lower_hex(
            "payload.administrator_public_key",
            &self.administrator_public_key,
            64,
        )?;
        if self.administrator_algorithm != "ed25519" {
            return Err("payload.administrator_algorithm must be ed25519".to_string());
        }

        let computed_dependency_root = exact_dependency_root(&self.dependencies)?;
        if computed_dependency_root != self.dependency_root {
            return Err(
                "payload.dependency_root does not match the canonical dependency list".to_string(),
            );
        }

        match self.mode {
            FrontierRepositoryBoundaryMode::TemporalizeExisting => {
                if self.trust_mode != FrontierRepositoryTrustMode::Tofu {
                    return Err("temporalize_existing requires trust_mode tofu".to_string());
                }
                if self.previous_identity_event_root.is_some() {
                    return Err(
                        "temporalize_existing requires a null previous_identity_event_root"
                            .to_string(),
                    );
                }
                let legacy_root =
                    self.legacy_identity_preimage_root
                        .as_deref()
                        .ok_or_else(|| {
                            "temporalize_existing requires legacy_identity_preimage_root"
                                .to_string()
                        })?;
                let legacy_origin = LegacyFrontierOriginV1 {
                    schema: LEGACY_FRONTIER_ORIGIN_SCHEMA.to_string(),
                    frontier_id: self.frontier_id.clone(),
                    legacy_identity_preimage_root: legacy_root.to_string(),
                    git_object_format: self.git_object_format,
                    anchor_git_commit: self.anchor_git_commit.clone(),
                    anchor_git_tree: self.anchor_git_tree.clone(),
                    anchor_event_log_root: self.anchor_event_log_root.clone(),
                    anchor_event_count: self.anchor_event_count,
                };
                if legacy_origin.identity_root()? != self.identity_root {
                    return Err(
                        "payload.identity_root does not match the temporalized legacy identity"
                            .to_string(),
                    );
                }
            }
            FrontierRepositoryBoundaryMode::UpdateDependencies => {
                if !matches!(
                    self.trust_mode,
                    FrontierRepositoryTrustMode::Genesis
                        | FrontierRepositoryTrustMode::PreviousBoundary
                ) {
                    return Err(
                        "update_dependencies requires trust_mode genesis or previous_boundary"
                            .to_string(),
                    );
                }
                if self.previous_identity_event_root.is_none() {
                    return Err(
                        "update_dependencies requires previous_identity_event_root".to_string()
                    );
                }
                if self.trust_mode == FrontierRepositoryTrustMode::Genesis
                    && self.legacy_identity_preimage_root.is_some()
                {
                    return Err(
                        "a genesis-chained dependency update cannot carry a legacy identity root"
                            .to_string(),
                    );
                }
            }
        }
        Ok(())
    }

    /// Check identity continuity against either the exact genesis event or the
    /// exact preceding repository-boundary event.
    ///
    /// This does not verify actor-registry authority or anchored Git history.
    pub fn validate_chain(&self, previous_event: &StateEvent) -> Result<(), String> {
        self.validate()?;
        if self.mode != FrontierRepositoryBoundaryMode::UpdateDependencies {
            return Err("only update_dependencies has a previous identity chain".to_string());
        }
        let previous_root = repository_identity_event_content_root(previous_event)?;
        if self.previous_identity_event_root.as_deref() != Some(previous_root.as_str()) {
            return Err(
                "previous_identity_event_root does not match the preceding event preimage"
                    .to_string(),
            );
        }
        if previous_event.kind.as_str() == "frontier.created" {
            if self.trust_mode != FrontierRepositoryTrustMode::Genesis {
                return Err("a frontier.created parent requires trust_mode genesis".to_string());
            }
            let identity = FrontierIdentityV1::from_genesis_event(previous_event)?;
            if self.frontier_id != identity.frontier_id
                || self.identity_root != identity.root()?
                || self.legacy_identity_preimage_root.is_some()
            {
                return Err(
                    "genesis-chained boundary does not preserve the derived Frontier identity"
                        .to_string(),
                );
            }
        } else {
            if self.trust_mode != FrontierRepositoryTrustMode::PreviousBoundary {
                return Err(
                    "a repository-boundary parent requires trust_mode previous_boundary"
                        .to_string(),
                );
            }
            let previous = repository_boundary_payload_from_event_shape(previous_event)?;
            if self.anchor_event_count <= previous.anchor_event_count {
                return Err(
                    "repository boundary anchor_event_count must advance beyond its parent"
                        .to_string(),
                );
            }
            if self.frontier_id != previous.frontier_id
                || self.identity_root != previous.identity_root
                || self.legacy_identity_preimage_root != previous.legacy_identity_preimage_root
                || self.administrator_actor_id != previous.administrator_actor_id
                || self.administrator_public_key != previous.administrator_public_key
                || self.administrator_algorithm != previous.administrator_algorithm
            {
                return Err(
                    "repository boundary chain changed immutable identity or administrator fields"
                        .to_string(),
                );
            }
        }
        Ok(())
    }
}

pub fn new_repository_boundary_event(
    payload: FrontierRepositoryBoundaryPayloadV1,
    reason: &str,
    timestamp: &str,
) -> Result<StateEvent, String> {
    payload.validate()?;
    require_text("event.reason", reason)?;
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map_err(|error| format!("event.timestamp must be RFC3339: {error}"))?;
    let mut event = StateEvent {
        schema: EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: EVENT_KIND_FRONTIER_REPOSITORY_BOUND.into(),
        target: StateTarget {
            r#type: "frontier".to_string(),
            id: payload.frontier_id.clone(),
        },
        actor: StateActor {
            r#type: "human".to_string(),
            id: payload.administrator_actor_id.clone(),
        },
        timestamp: timestamp.to_string(),
        reason: reason.to_string(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: serde_json::to_value(payload)
            .map_err(|error| format!("serialize repository-boundary payload: {error}"))?,
        caveats: vec![],
        signature: None,
    };
    event.id = compute_event_id(&event);
    Ok(event)
}

/// Parse and validate the event's closed payload and fixed core shape.
///
/// This checks that a signature is present but does **not** verify it and does
/// not establish Git-anchor or actor-registry authority.
pub fn repository_boundary_payload_from_event_shape(
    event: &StateEvent,
) -> Result<FrontierRepositoryBoundaryPayloadV1, String> {
    if event.schema != EVENT_SCHEMA {
        return Err(format!(
            "repository boundary event schema must be {EVENT_SCHEMA}"
        ));
    }
    if event.kind.as_str() != EVENT_KIND_FRONTIER_REPOSITORY_BOUND {
        return Err(format!(
            "expected {EVENT_KIND_FRONTIER_REPOSITORY_BOUND}, got {}",
            event.kind
        ));
    }
    if event.id != compute_event_id(event) {
        return Err(
            "repository boundary event id does not match its canonical content".to_string(),
        );
    }
    if event.target.r#type != "frontier" {
        return Err("repository boundary target.type must be frontier".to_string());
    }
    if event.actor.r#type != "human" {
        return Err("repository boundary actor.type must be human".to_string());
    }
    if event.before_hash != NULL_HASH || event.after_hash != NULL_HASH {
        return Err("repository boundary must use null before_hash and after_hash".to_string());
    }
    require_text("event.reason", &event.reason)?;
    chrono::DateTime::parse_from_rfc3339(&event.timestamp)
        .map_err(|error| format!("event.timestamp must be RFC3339: {error}"))?;
    if event.signature.is_none() {
        return Err("repository boundary must carry an ordinary event signature".to_string());
    }
    let payload: FrontierRepositoryBoundaryPayloadV1 =
        serde_json::from_value(event.payload.clone())
            .map_err(|error| format!("invalid repository-boundary payload: {error}"))?;
    payload.validate()?;
    if event.target.id != payload.frontier_id {
        return Err("repository boundary target.id must equal payload.frontier_id".to_string());
    }
    if event.actor.id != payload.administrator_actor_id {
        return Err(
            "repository boundary actor.id must equal payload.administrator_actor_id".to_string(),
        );
    }
    Ok(payload)
}

/// Verify only cryptographic proof of possession for the expected key.
///
/// The function name is intentionally explicit: success does not establish
/// that the key was the active administrator at the anchored history, that the
/// Git anchor exists, or that retained bytes match the payload. Those checks
/// belong to the future anchor-context verifier.
pub fn verify_repository_boundary_signature_only(
    event: &StateEvent,
    expected_public_key: &str,
) -> Result<FrontierRepositoryBoundaryPayloadV1, String> {
    require_lower_hex("expected administrator public key", expected_public_key, 64)?;
    let payload = repository_boundary_payload_from_event_shape(event)?;
    if payload.administrator_public_key != expected_public_key {
        return Err(
            "repository boundary payload key does not match the expected administrator key"
                .to_string(),
        );
    }
    if !crate::sign::verify_event_signature(event, expected_public_key)? {
        return Err("repository boundary event signature does not verify".to_string());
    }
    Ok(payload)
}

pub fn repository_boundary_event_content_root(event: &StateEvent) -> Result<String, String> {
    repository_boundary_payload_from_event_shape(event)?;
    Ok(event_content_root(event))
}

/// Full content root of either repository identity event kind.
pub fn repository_identity_event_content_root(event: &StateEvent) -> Result<String, String> {
    match event.kind.as_str() {
        "frontier.created" => {
            validate_profile_v1_genesis_event(event)?;
            Ok(event_content_root(event))
        }
        EVENT_KIND_FRONTIER_REPOSITORY_BOUND => repository_boundary_event_content_root(event),
        other => Err(format!(
            "repository identity parent must be frontier.created or {EVENT_KIND_FRONTIER_REPOSITORY_BOUND}, got {other}"
        )),
    }
}

/// Validate the complete repository-boundary graph present in one event set.
///
/// This is intentionally independent of event timestamps. It proves event
/// shape, proof of key possession, one linear identity chain, and
/// payload-local continuity. Git-anchor membership and actor-registry
/// authority require repository context and are checked by the higher layer.
pub fn validate_repository_boundary_event_set(events: &[StateEvent]) -> Vec<String> {
    struct Boundary<'a> {
        event: &'a StateEvent,
        payload: FrontierRepositoryBoundaryPayloadV1,
    }

    let mut boundaries = BTreeMap::<String, Boundary<'_>>::new();
    let mut errors = Vec::new();

    for event in events
        .iter()
        .filter(|event| event.kind.as_str() == EVENT_KIND_FRONTIER_REPOSITORY_BOUND)
    {
        let payload = match repository_boundary_payload_from_event_shape(event) {
            Ok(payload) => payload,
            Err(error) => {
                errors.push(format!("repository boundary {} invalid: {error}", event.id));
                continue;
            }
        };
        if let Err(error) =
            verify_repository_boundary_signature_only(event, &payload.administrator_public_key)
        {
            errors.push(format!(
                "repository boundary {} signature invalid: {error}",
                event.id
            ));
            continue;
        }
        let root = event_content_root(event);
        if boundaries
            .insert(root.clone(), Boundary { event, payload })
            .is_some()
        {
            errors.push(format!("duplicate repository boundary content root {root}"));
        }
    }

    if boundaries.is_empty() {
        return errors;
    }

    let mut genesis = BTreeMap::new();
    for event in events.iter().filter(|event| {
        event.kind.as_str() == "frontier.created"
            && event
                .payload
                .get("schema")
                .and_then(serde_json::Value::as_str)
                == Some(FRONTIER_CREATED_SCHEMA_V1)
    }) {
        match repository_identity_event_content_root(event) {
            Ok(root) => {
                if genesis.insert(root.clone(), event).is_some() {
                    errors.push(format!("duplicate frontier.created content root {root}"));
                }
            }
            Err(error) => errors.push(format!("frontier.created {} invalid: {error}", event.id)),
        }
    }
    if genesis.len() > 1 {
        errors.push(format!(
            "repository boundary graph cannot contain {} distinct frontier.created roots",
            genesis.len()
        ));
    }

    let mut roots = 0usize;
    let mut children = BTreeMap::<String, Vec<String>>::new();
    for (root, boundary) in &boundaries {
        match boundary.payload.mode {
            FrontierRepositoryBoundaryMode::TemporalizeExisting => {
                roots += 1;
                if !genesis.is_empty() {
                    errors.push(format!(
                        "legacy temporal boundary {root} cannot coexist with frontier.created"
                    ));
                }
            }
            FrontierRepositoryBoundaryMode::UpdateDependencies => {
                let Some(parent_root) = boundary.payload.previous_identity_event_root.as_deref()
                else {
                    // Intrinsic validation already reports this, but retain a
                    // defensive branch for a future parser change.
                    errors.push(format!("repository boundary {root} has no identity parent"));
                    continue;
                };
                children
                    .entry(parent_root.to_string())
                    .or_default()
                    .push(root.clone());

                let parent = boundaries
                    .get(parent_root)
                    .map(|parent| parent.event)
                    .or_else(|| genesis.get(parent_root).copied());
                let Some(parent) = parent else {
                    errors.push(format!(
                        "repository boundary {root} references missing identity parent {parent_root}"
                    ));
                    continue;
                };
                if parent.kind.as_str() == "frontier.created" {
                    roots += 1;
                }
                if let Err(error) = boundary.payload.validate_chain(parent) {
                    errors.push(format!("repository boundary {root} chain invalid: {error}"));
                }
            }
        }
    }

    for (parent, child_roots) in &children {
        if child_roots.len() > 1 {
            errors.push(format!(
                "repository identity event {parent} has conflicting children {}",
                child_roots.join(", ")
            ));
        }
    }
    if roots != 1 {
        errors.push(format!(
            "repository boundary graph must have exactly one identity-chain root, found {roots}"
        ));
    }

    for start in boundaries.keys() {
        let mut current = start.as_str();
        let mut visited = BTreeSet::new();
        loop {
            if !visited.insert(current.to_string()) {
                errors.push(format!(
                    "repository boundary graph contains a cycle through {current}"
                ));
                break;
            }
            let Some(boundary) = boundaries.get(current) else {
                break;
            };
            if boundary.payload.mode == FrontierRepositoryBoundaryMode::TemporalizeExisting {
                break;
            }
            let Some(parent) = boundary.payload.previous_identity_event_root.as_deref() else {
                break;
            };
            if genesis.contains_key(parent) {
                break;
            }
            current = parent;
        }
    }

    errors.sort();
    errors.dedup();
    errors
}

fn validate_genesis_event(event: &StateEvent) -> Result<(), String> {
    if event.schema != EVENT_SCHEMA {
        return Err(format!("genesis event schema must be {EVENT_SCHEMA}"));
    }
    if event.kind.as_str() != "frontier.created" {
        return Err(format!("expected frontier.created, got {}", event.kind));
    }
    if event.id != compute_event_id(event) {
        return Err("frontier.created id does not match its canonical content".to_string());
    }
    if event.target.r#type != "frontier" || event.actor.r#type != "frontier" {
        return Err("frontier.created target and actor types must be frontier".to_string());
    }
    if event.before_hash != NULL_HASH || event.after_hash != NULL_HASH {
        return Err("frontier.created must use null before_hash and after_hash".to_string());
    }
    crate::events::validate_event_payload("frontier.created", &event.payload)?;
    let payload = event
        .payload
        .as_object()
        .ok_or_else(|| "frontier.created payload must be an object".to_string())?;
    if payload.get("schema").and_then(serde_json::Value::as_str) == Some(FRONTIER_CREATED_SCHEMA_V1)
    {
        if event.signature.is_some() || !event.caveats.is_empty() {
            return Err(
                "Profile v1 frontier.created must be unsigned and carry no caveats".to_string(),
            );
        }
        if payload
            .get("name_at_creation")
            .and_then(serde_json::Value::as_str)
            != Some(event.target.id.as_str())
            || payload.get("creator").and_then(serde_json::Value::as_str)
                != Some(event.actor.id.as_str())
            || payload
                .get("created_at")
                .and_then(serde_json::Value::as_str)
                != Some(event.timestamp.as_str())
        {
            return Err(
                "Profile v1 frontier.created payload identity disagrees with its event core"
                    .to_string(),
            );
        }
        let empty_dependency_root = exact_dependency_root(&[])?;
        if payload
            .get("dependency_root")
            .and_then(serde_json::Value::as_str)
            != Some(empty_dependency_root.as_str())
        {
            return Err(
                "Profile v1 frontier.created must bind the canonical empty dependency root"
                    .to_string(),
            );
        }
    } else {
        if payload.get("name").and_then(serde_json::Value::as_str) != Some(event.target.id.as_str())
            || payload.get("creator").and_then(serde_json::Value::as_str)
                != Some(event.actor.id.as_str())
        {
            return Err(
                "historical frontier.created payload identity disagrees with its event core"
                    .to_string(),
            );
        }
        if let Some(compiled_at) = payload
            .get("compiled_at")
            .and_then(serde_json::Value::as_str)
            && compiled_at != event.timestamp
        {
            return Err(
                "historical frontier.created compiled_at disagrees with event timestamp"
                    .to_string(),
            );
        }
    }
    Ok(())
}

/// A Profile v1 repository identity may only originate in the closed v1
/// structural genesis. Historical `frontier.created` events remain valid
/// replay input, but they cannot be relabeled as the stronger v1 identity
/// source or used to bypass a protected legacy-boundary migration.
fn validate_profile_v1_genesis_event(event: &StateEvent) -> Result<(), String> {
    validate_genesis_event(event)?;
    if event
        .payload
        .get("schema")
        .and_then(serde_json::Value::as_str)
        != Some(FRONTIER_CREATED_SCHEMA_V1)
    {
        return Err(format!(
            "repository identity genesis must use payload.schema {FRONTIER_CREATED_SCHEMA_V1}"
        ));
    }
    Ok(())
}

fn event_content_root(event: &StateEvent) -> String {
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(event_content_preimage_bytes(event)))
    )
}

fn canonical_root<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        crate::canonical::sha256_canonical(value)?
    ))
}

fn require_frontier_id(field: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("vfr_") else {
        return Err(format!("{field} must use the vfr_<16 lowercase hex> form"));
    };
    require_lower_hex(field, hex, 16)
}

fn require_git_object(
    field: &str,
    value: &str,
    object_format: GitObjectFormat,
) -> Result<(), String> {
    require_lower_hex(field, value, object_format.digest_len())
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must be non-empty"));
    }
    if value.nfc().collect::<String>() != value {
        return Err(format!("{field} must already be Unicode NFC"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} contains a forbidden control character"));
    }
    Ok(())
}

fn validate_repository_path(path: &str) -> Result<(), String> {
    require_text("retained object path", path)?;
    if path.starts_with('/') || path.ends_with('/') || path.contains('\\') {
        return Err(
            "retained object path must be a normalized relative repository path".to_string(),
        );
    }
    if path
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(
            "retained object path contains an empty, dot, or traversal segment".to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    #[test]
    fn frontier_repository_bound_legacy_identity_root_is_deterministic() {
        let origin = LegacyFrontierOriginV1 {
            schema: LEGACY_FRONTIER_ORIGIN_SCHEMA.to_string(),
            frontier_id: "vfr_1234567890abcdef".to_string(),
            legacy_identity_preimage_root: root('1'),
            git_object_format: GitObjectFormat::Sha1,
            anchor_git_commit: "2".repeat(40),
            anchor_git_tree: "3".repeat(40),
            anchor_event_log_root: root('4'),
            anchor_event_count: 1,
        };
        let identity = origin.identity().unwrap();
        let expected = canonical_root(&identity).unwrap();
        assert_eq!(origin.identity_root().unwrap(), expected);
        identity.verify_root(&expected).unwrap();

        let mut changed = origin;
        changed.anchor_event_count = 2;
        assert_ne!(changed.identity_root().unwrap(), expected);
    }

    #[test]
    fn frontier_identity_record_shape_rejects_a_genesis_legacy_root() {
        let valid = FrontierIdentityV1 {
            schema: FRONTIER_IDENTITY_SCHEMA.to_string(),
            frontier_id: "vfr_1234567890abcdef".to_string(),
            origin: FrontierIdentityOrigin::Genesis,
            origin_commitment: root('1'),
            legacy_identity_preimage_root: None,
        };
        assert!(valid.root().is_ok());

        let mut invalid = valid;
        invalid.legacy_identity_preimage_root = Some(root('2'));
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn frontier_repository_bound_manifest_wire_value_is_the_exact_list() {
        let manifest = RetainedObjectManifestV1(vec![RetainedObjectEntryV1 {
            path: "records/receipt.json".to_string(),
            git_mode: "100644".to_string(),
            size: 7,
            sha256: "a".repeat(64),
        }]);
        let value = serde_json::to_value(&manifest).unwrap();
        assert!(value.is_array());
        assert_eq!(
            manifest.root().unwrap(),
            canonical_root(&manifest.0).unwrap()
        );
    }
}
