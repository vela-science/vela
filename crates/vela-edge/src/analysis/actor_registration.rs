//! Git-rooted assessment of temporal actor-registration events.
//!
//! This is a derived check over canonical events and immutable Git objects. It
//! never mutates the frontier and never treats timestamps as membership.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vela_protocol::actor_registration::{
    ACTOR_REGISTRATION_BOUNDARY_SCHEMA, ActorRegistrationAnchor, ActorRegistrationBoundaryPayload,
    ActorRegistrationMode, payload_from_event, verify_activation_signature,
};
use vela_protocol::events::{self, StateEvent};
use vela_protocol::project::Project;
use vela_protocol::sign::ActorRecord;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnchoredActorEvent {
    pub content_preimage_sha256: String,
    pub signature_was_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryOutcome {
    Valid,
    Invalid,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorBoundaryAssessment {
    pub actor_id: String,
    pub activation_event_id: String,
    pub outcome: BoundaryOutcome,
    pub reason: Option<String>,
    pub anchored_events: BTreeMap<String, AnchoredActorEvent>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorRegistrationReport {
    pub boundaries: BTreeMap<String, ActorBoundaryAssessment>,
    pub removed_activation_event_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorRegistrationPreview {
    pub payload: ActorRegistrationBoundaryPayload,
    pub anchored_unsigned: usize,
    pub anchored_signed: usize,
    pub post_anchor_unsigned: usize,
    pub post_anchor_signed: usize,
}

impl ActorRegistrationReport {
    pub fn boundary(&self, actor_id: &str) -> Option<&ActorBoundaryAssessment> {
        self.boundaries.get(actor_id)
    }
}

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = super::git_read::hardened_command(repo, "actor-registration Git repository")?
        .args(args)
        .output()
        .map_err(|error| format!("failed to run git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("git {} failed with {}", args.join(" "), output.status)
        } else {
            stderr
        });
    }
    Ok(output.stdout)
}

fn git_succeeds(repo: &Path, args: &[&str]) -> Result<bool, String> {
    super::git_read::hardened_command(repo, "actor-registration Git repository")?
        .args(args)
        .status()
        .map(|status| status.success())
        .map_err(|error| format!("failed to run git {}: {error}", args.join(" ")))
}

fn git_text(repo: &Path, args: &[&str]) -> Result<String, String> {
    String::from_utf8(git(repo, args)?)
        .map(|text| text.trim().to_string())
        .map_err(|error| format!("git output was not UTF-8: {error}"))
}

fn sha256_root(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn event_preimage_root(event: &StateEvent) -> String {
    sha256_root(&events::event_content_preimage_bytes(event))
}

fn anchor_events(repo: &Path, commit: &str) -> Result<Vec<StateEvent>, String> {
    let paths = git_text(
        repo,
        &["ls-tree", "-r", "--name-only", commit, "--", ".vela/events"],
    )?;
    let mut parsed = Vec::new();
    for path in paths.lines().filter(|path| path.ends_with(".json")) {
        let object = format!("{commit}:{path}");
        let bytes = git(repo, &["show", &object])?;
        let event: StateEvent = serde_json::from_slice(&bytes)
            .map_err(|error| format!("failed to parse anchored event {path}: {error}"))?;
        if events::compute_event_id(&event) != event.id {
            return Err(format!(
                "anchored event {} does not match its content-addressed id",
                event.id
            ));
        }
        parsed.push(event);
    }
    Ok(parsed)
}

fn anchor_registry(repo: &Path, commit: &str) -> Result<(Vec<u8>, Vec<ActorRecord>), String> {
    let object = format!("{commit}:.vela/actors.json");
    let bytes = git(repo, &["show", &object])?;
    let actors = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse anchored actor registry: {error}"))?;
    Ok((bytes, actors))
}

fn current_actor<'a>(
    frontier: &'a Project,
    payload: &ActorRegistrationBoundaryPayload,
) -> Result<&'a ActorRecord, String> {
    let matches = frontier
        .actors
        .iter()
        .filter(|actor| actor.id == payload.actor_id)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!(
            "current actor registry must contain exactly one record for {}, found {}",
            payload.actor_id,
            matches.len()
        ));
    }
    let actor = matches[0];
    if actor.public_key != payload.public_key || actor.algorithm != payload.algorithm {
        return Err("current actor registry does not match the activated key binding".to_string());
    }
    Ok(actor)
}

fn assess_one(
    frontier: &Project,
    repo: &Path,
    activation: &StateEvent,
) -> Result<BTreeMap<String, AnchoredActorEvent>, String> {
    let payload = payload_from_event(activation)?;
    if frontier.frontier_id() != payload.frontier_id {
        return Err(format!(
            "activation frontier_id {} does not match current {}",
            payload.frontier_id,
            frontier.frontier_id()
        ));
    }
    let resolved_commit = git_text(
        repo,
        &[
            "rev-parse",
            &format!("{}^{{commit}}", payload.anchor.git_commit),
        ],
    )
    .map_err(|error| format!("anchor unavailable: {error}"))?;
    if resolved_commit != payload.anchor.git_commit {
        return Err("anchor commit did not resolve to the exact signed object".to_string());
    }
    let head = git_text(repo, &["rev-parse", "HEAD^{commit}"])
        .map_err(|error| format!("checked revision unavailable: {error}"))?;
    if !git_succeeds(
        repo,
        &["merge-base", "--is-ancestor", &resolved_commit, &head],
    )? {
        return Err("anchor commit is not an ancestor of the checked revision".to_string());
    }
    let tree = git_text(repo, &["show", "-s", "--format=%T", &resolved_commit])?;
    if tree != payload.anchor.git_tree {
        return Err(format!(
            "anchor tree mismatch: signed {}, resolved {tree}",
            payload.anchor.git_tree
        ));
    }

    let anchored_events = anchor_events(repo, &resolved_commit)
        .map_err(|error| format!("anchor unavailable: {error}"))?;
    if anchored_events.len() as u64 != payload.anchor.event_count {
        return Err(format!(
            "anchor event count mismatch: signed {}, resolved {}",
            payload.anchor.event_count,
            anchored_events.len()
        ));
    }
    let event_root = format!("sha256:{}", events::event_log_hash(&anchored_events));
    if event_root != payload.anchor.event_log_root {
        return Err(format!(
            "anchor event-log root mismatch: signed {}, resolved {event_root}",
            payload.anchor.event_log_root
        ));
    }

    let (registry_bytes, anchored_registry) = anchor_registry(repo, &resolved_commit)
        .map_err(|error| format!("anchor unavailable: {error}"))?;
    let registry_root = sha256_root(&registry_bytes);
    if registry_root != payload.anchor.actor_registry_root {
        return Err(format!(
            "anchor actor-registry root mismatch: signed {}, resolved {registry_root}",
            payload.anchor.actor_registry_root
        ));
    }

    let verification_key = match payload.mode {
        ActorRegistrationMode::TemporalizeExisting => {
            let matches = anchored_registry
                .iter()
                .filter(|actor| actor.id == payload.actor_id)
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(format!(
                    "anchored registry must contain exactly one record for {}, found {}",
                    payload.actor_id,
                    matches.len()
                ));
            }
            let actor = matches[0];
            if actor.public_key != payload.public_key || actor.algorithm != payload.algorithm {
                return Err(
                    "anchored actor record does not match the activation key binding".to_string(),
                );
            }
            actor.public_key.as_str()
        }
        ActorRegistrationMode::Bootstrap => {
            if !anchored_registry.is_empty() {
                return Err("bootstrap requires an empty anchored actor registry".to_string());
            }
            payload.public_key.as_str()
        }
    };
    verify_activation_signature(activation, verification_key)?;
    current_actor(frontier, &payload)?;

    let current_by_id = frontier
        .events
        .iter()
        .map(|event| (event.id.as_str(), event))
        .collect::<BTreeMap<_, _>>();
    let mut membership = BTreeMap::new();
    for anchored in anchored_events {
        let Some(current) = current_by_id.get(anchored.id.as_str()) else {
            return Err(format!("anchored event {} is missing", anchored.id));
        };
        let anchored_preimage = event_preimage_root(&anchored);
        if event_preimage_root(current) != anchored_preimage {
            return Err(format!(
                "anchored event {} changed canonical content",
                anchored.id
            ));
        }
        membership.insert(
            anchored.id,
            AnchoredActorEvent {
                content_preimage_sha256: anchored_preimage,
                signature_was_present: anchored.signature.is_some(),
            },
        );
    }
    Ok(membership)
}

fn historical_activation_ids(repo: &Path) -> Result<BTreeSet<String>, String> {
    let commits = git_text(
        repo,
        &[
            "log",
            "--format=%H",
            "-G",
            "actor.registration_activated",
            "HEAD",
            "--",
            ".vela/events",
        ],
    )?;
    let mut ids = BTreeSet::new();
    for commit in commits.lines() {
        for event in anchor_events(repo, commit).unwrap_or_default() {
            if event.kind.as_str() == events::EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED
                && payload_from_event(&event).is_ok()
            {
                ids.insert(event.id);
            }
        }
    }
    Ok(ids)
}

pub fn preview_temporalize_existing(
    frontier: &Project,
    repo: &Path,
    actor_id: &str,
    requested_anchor: &str,
) -> Result<ActorRegistrationPreview, String> {
    if frontier.events.iter().any(|event| {
        event.kind.as_str() == events::EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED
            && event
                .payload
                .get("actor_id")
                .and_then(serde_json::Value::as_str)
                == Some(actor_id)
    }) {
        return Err(format!(
            "actor {actor_id} already has an activation event; duplicate boundaries fail closed"
        ));
    }
    let resolved_commit = git_text(
        repo,
        &["rev-parse", &format!("{requested_anchor}^{{commit}}")],
    )
    .map_err(|error| format!("anchor unavailable: {error}"))?;
    let head = git_text(repo, &["rev-parse", "HEAD^{commit}"])?;
    if !git_succeeds(
        repo,
        &["merge-base", "--is-ancestor", &resolved_commit, &head],
    )? {
        return Err("anchor commit is not an ancestor of the checked revision".to_string());
    }
    let tree = git_text(repo, &["show", "-s", "--format=%T", &resolved_commit])?;
    let anchored_events = anchor_events(repo, &resolved_commit)?;
    let (registry_bytes, anchored_registry) = anchor_registry(repo, &resolved_commit)?;
    let matches = anchored_registry
        .iter()
        .filter(|actor| actor.id == actor_id)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!(
            "anchored registry must contain exactly one record for {actor_id}, found {}",
            matches.len()
        ));
    }
    let anchored_actor = matches[0];
    let current_matches = frontier
        .actors
        .iter()
        .filter(|actor| actor.id == actor_id)
        .collect::<Vec<_>>();
    if current_matches.len() != 1 {
        return Err(format!(
            "current registry must contain exactly one record for {actor_id}, found {}",
            current_matches.len()
        ));
    }
    let current_actor = current_matches[0];
    if current_actor.public_key != anchored_actor.public_key
        || current_actor.algorithm != anchored_actor.algorithm
    {
        return Err("current actor record does not match the anchored registry".to_string());
    }
    let anchor_ids = anchored_events
        .iter()
        .map(|event| event.id.as_str())
        .collect::<BTreeSet<_>>();
    let anchored_unsigned = anchored_events
        .iter()
        .filter(|event| event.actor.id == actor_id && event.signature.is_none())
        .count();
    let anchored_signed = anchored_events
        .iter()
        .filter(|event| event.actor.id == actor_id && event.signature.is_some())
        .count();
    let post_anchor_unsigned = frontier
        .events
        .iter()
        .filter(|event| {
            event.actor.id == actor_id
                && !anchor_ids.contains(event.id.as_str())
                && event.signature.is_none()
        })
        .count();
    let post_anchor_signed = frontier
        .events
        .iter()
        .filter(|event| {
            event.actor.id == actor_id
                && !anchor_ids.contains(event.id.as_str())
                && event.signature.is_some()
        })
        .count();
    let payload = ActorRegistrationBoundaryPayload {
        schema: ACTOR_REGISTRATION_BOUNDARY_SCHEMA.to_string(),
        mode: ActorRegistrationMode::TemporalizeExisting,
        frontier_id: frontier.frontier_id(),
        actor_id: actor_id.to_string(),
        public_key: anchored_actor.public_key.clone(),
        algorithm: anchored_actor.algorithm.clone(),
        anchor: ActorRegistrationAnchor {
            git_object_format: if resolved_commit.len() == 40 {
                "sha1".to_string()
            } else {
                "sha256".to_string()
            },
            git_commit: resolved_commit,
            git_tree: tree,
            event_log_root: format!("sha256:{}", events::event_log_hash(&anchored_events)),
            event_count: anchored_events.len() as u64,
            actor_registry_root: sha256_root(&registry_bytes),
        },
    };
    payload.validate()?;
    Ok(ActorRegistrationPreview {
        payload,
        anchored_unsigned,
        anchored_signed,
        post_anchor_unsigned,
        post_anchor_signed,
    })
}

pub fn assess(frontier: &Project, repo: Option<&Path>) -> ActorRegistrationReport {
    let activations = frontier
        .events
        .iter()
        .filter(|event| event.kind.as_str() == events::EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED)
        .collect::<Vec<_>>();
    let mut by_actor = BTreeMap::<String, Vec<&StateEvent>>::new();
    for activation in &activations {
        let actor_id = activation
            .payload
            .get("actor_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&activation.actor.id)
            .to_string();
        by_actor.entry(actor_id).or_default().push(activation);
    }

    let mut report = ActorRegistrationReport::default();
    for (actor_id, events) in by_actor {
        if events.len() != 1 {
            report.boundaries.insert(
                actor_id.clone(),
                ActorBoundaryAssessment {
                    actor_id,
                    activation_event_id: events[0].id.clone(),
                    outcome: BoundaryOutcome::Invalid,
                    reason: Some(format!(
                        "duplicate actor-registration boundaries: {}",
                        events.len()
                    )),
                    anchored_events: BTreeMap::new(),
                },
            );
            continue;
        }
        let activation = events[0];
        let Some(repo) = repo else {
            report.boundaries.insert(
                actor_id.clone(),
                ActorBoundaryAssessment {
                    actor_id,
                    activation_event_id: activation.id.clone(),
                    outcome: BoundaryOutcome::Unavailable,
                    reason: Some(
                        "Git repository context is required to resolve the signed anchor"
                            .to_string(),
                    ),
                    anchored_events: BTreeMap::new(),
                },
            );
            continue;
        };
        match assess_one(frontier, repo, activation) {
            Ok(anchored_events) => {
                report.boundaries.insert(
                    actor_id.clone(),
                    ActorBoundaryAssessment {
                        actor_id,
                        activation_event_id: activation.id.clone(),
                        outcome: BoundaryOutcome::Valid,
                        reason: None,
                        anchored_events,
                    },
                );
            }
            Err(reason) => {
                let unavailable = reason.starts_with("anchor unavailable:")
                    || reason.starts_with("checked revision unavailable:");
                report.boundaries.insert(
                    actor_id.clone(),
                    ActorBoundaryAssessment {
                        actor_id,
                        activation_event_id: activation.id.clone(),
                        outcome: if unavailable {
                            BoundaryOutcome::Unavailable
                        } else {
                            BoundaryOutcome::Invalid
                        },
                        reason: Some(reason),
                        anchored_events: BTreeMap::new(),
                    },
                );
            }
        }
    }

    if let Some(repo) = repo
        && let Ok(historical) = historical_activation_ids(repo)
    {
        let current = activations
            .iter()
            .map(|event| event.id.as_str())
            .collect::<BTreeSet<_>>();
        report.removed_activation_event_ids = historical
            .into_iter()
            .filter(|id| !current.contains(id.as_str()))
            .collect();
    }
    report
}
