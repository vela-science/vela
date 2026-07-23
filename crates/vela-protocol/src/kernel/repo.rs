//! Git-native VelaRepo abstraction — load/save projects from either monolithic JSON
//! or a `.vela/` directory of individual finding files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::bundle::{ConfidenceUpdate, FindingBundle, ReviewEvent};
use crate::events::StateEvent;
use crate::project::{self, Project};
use crate::proposals::{ProofState, StateProposal};
use crate::reducer;

// ── Source detection ──────────────────────────────────────────────────

/// Where a project lives on disk.
#[derive(Debug, Clone, PartialEq)]
pub enum VelaSource {
    /// A single monolithic JSON file.
    ProjectFile(PathBuf),
    /// A directory with a `.vela/` subdirectory containing individual finding files.
    VelaRepo(PathBuf),
    /// A publishable frontier packet directory with `manifest.json` and payload files.
    PacketDir(PathBuf),
}

#[derive(Debug, Deserialize)]
struct PacketManifestHeader {
    packet_format: String,
    #[serde(default)]
    source: Option<PacketSourceHeader>,
}

#[derive(Debug, Default, Deserialize)]
struct PacketSourceHeader {
    #[serde(default)]
    project_name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    compiled_at: String,
    #[serde(default)]
    compiler: String,
    #[serde(default)]
    vela_version: String,
    #[serde(default)]
    schema: String,
}

#[derive(Debug, Default, Deserialize)]
struct PacketOverviewHeader {
    #[serde(default)]
    project_name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    compiled_at: String,
    #[serde(default)]
    papers_processed: usize,
}

/// Detect the source type from a path.
///
/// - If `path` points to a file with `.json` extension -> ProjectFile
/// - If `path` is a directory with a `.vela/` subdirectory -> VelaRepo
/// - Otherwise -> error
pub fn detect(path: &Path) -> Result<VelaSource, String> {
    if path.is_file() {
        return Ok(VelaSource::ProjectFile(path.to_path_buf()));
    }
    if path.is_dir() {
        if is_packet_dir(path) {
            return Ok(VelaSource::PacketDir(path.to_path_buf()));
        }
        let vela_dir = path.join(".vela");
        if vela_dir.is_dir() {
            return Ok(VelaSource::VelaRepo(path.to_path_buf()));
        }
        // A path that looks like it should be a JSON file but doesn't exist yet
        if path.extension().is_some_and(|ext| ext == "json") {
            return Ok(VelaSource::ProjectFile(path.to_path_buf()));
        }
        return Err(format!(
            "Directory '{}' is not a Vela repository or frontier packet. Run `vela init` here, or clone an existing frontier's git repo.",
            path.display()
        ));
    }
    // Path doesn't exist yet — check extension
    if path.extension().is_some_and(|ext| ext == "json") {
        return Ok(VelaSource::ProjectFile(path.to_path_buf()));
    }
    Err(format!(
        "Path '{}' does not exist. Provide a .json file, frontier packet, or a directory with .vela/",
        path.display()
    ))
}

// ── Config TOML ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RepoConfig {
    project: RepoProjectMeta,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RepoProjectMeta {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    frontier_id: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    compiled_at: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_compiler")]
    compiler: String,
    #[serde(default)]
    papers_processed: usize,
}

fn default_compiler() -> String {
    crate::project::VELA_COMPILER_VERSION.into()
}

/// Render the legacy v0.1 mixed config without erasing operational or unknown
/// sections. If Vela-owned project metadata is unchanged, retain the original
/// bytes exactly (including comments and formatting). If it changed, merge only
/// the closed project fields into the parsed TOML tree and preserve every other
/// value structurally. Invalid existing TOML fails closed rather than being
/// replaced by a clean file that silently drops operator settings.
fn render_legacy_repo_config(config_path: &Path, desired: &RepoConfig) -> Result<Vec<u8>, String> {
    let fresh = || {
        toml::to_string_pretty(desired)
            .map(String::into_bytes)
            .map_err(|error| format!("Failed to serialize config.toml: {error}"))
    };
    let existing = match std::fs::read(config_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return fresh(),
        Err(error) => {
            return Err(format!(
                "Failed to read existing config.toml '{}': {error}",
                config_path.display()
            ));
        }
    };
    let existing_text = std::str::from_utf8(&existing).map_err(|error| {
        format!(
            "Existing config.toml '{}' is not UTF-8: {error}",
            config_path.display()
        )
    })?;
    let existing_project: RepoConfig = toml::from_str(existing_text).map_err(|error| {
        format!(
            "Existing config.toml '{}' cannot be decoded without data loss: {error}",
            config_path.display()
        )
    })?;
    if existing_project == *desired {
        return Ok(existing);
    }

    let mut merged: toml::Value = existing_text.parse().map_err(|error| {
        format!(
            "Existing config.toml '{}' cannot be parsed without data loss: {error}",
            config_path.display()
        )
    })?;
    let desired_value = toml::Value::try_from(desired)
        .map_err(|error| format!("Failed to encode desired config.toml fields: {error}"))?;
    let desired_project = desired_value
        .get("project")
        .and_then(toml::Value::as_table)
        .ok_or("serialized config.toml is missing [project]")?;
    let merged_root = merged
        .as_table_mut()
        .ok_or("existing config.toml root must be a table")?;
    let merged_project = merged_root
        .entry("project".to_string())
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or("existing config.toml [project] must be a table")?;

    for field in [
        "name",
        "frontier_id",
        "compiled_at",
        "description",
        "compiler",
        "papers_processed",
    ] {
        match desired_project.get(field) {
            Some(value) => {
                merged_project.insert(field.to_string(), value.clone());
            }
            None => {
                merged_project.remove(field);
            }
        }
    }

    toml::to_string_pretty(&merged)
        .map(String::into_bytes)
        .map_err(|error| format!("Failed to serialize merged config.toml: {error}"))
}

// ── Load ─────────────────────────────────────────────────────────────

/// Load a project from a detected source.
pub fn load(source: &VelaSource) -> Result<Project, String> {
    match source {
        VelaSource::ProjectFile(path) => load_project_file(path),
        VelaSource::VelaRepo(dir) => load_vela_repo(dir),
        VelaSource::PacketDir(dir) => load_packet_dir(dir),
    }
}

pub(crate) fn load_project_file(path: &Path) -> Result<Project, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read project file '{}': {e}", path.display()))?;
    serde_json::from_str(&data)
        .map_err(|e| format!("Failed to parse project JSON '{}': {e}", path.display()))
}

fn load_packet_dir(dir: &Path) -> Result<Project, String> {
    let manifest_path = dir.join("manifest.json");
    let manifest_data = std::fs::read_to_string(&manifest_path).map_err(|e| {
        format!(
            "Failed to read packet manifest '{}': {e}",
            manifest_path.display()
        )
    })?;
    let manifest: PacketManifestHeader = serde_json::from_str(&manifest_data).map_err(|e| {
        format!(
            "Failed to parse packet manifest '{}': {e}",
            manifest_path.display()
        )
    })?;

    if manifest.packet_format != "vela.frontier-packet" {
        return Err(format!(
            "Unsupported packet format '{}' in {}",
            manifest.packet_format,
            manifest_path.display()
        ));
    }

    let findings_path = dir.join("findings/full.json");
    let findings_data = std::fs::read_to_string(&findings_path).map_err(|e| {
        format!(
            "Failed to read packet findings '{}': {e}",
            findings_path.display()
        )
    })?;
    let findings: Vec<FindingBundle> = serde_json::from_str(&findings_data).map_err(|e| {
        format!(
            "Failed to parse packet findings '{}': {e}",
            findings_path.display()
        )
    })?;

    let reviews_path = dir.join("reviews/review-events.json");
    let review_events: Vec<ReviewEvent> = if reviews_path.is_file() {
        let reviews_data = std::fs::read_to_string(&reviews_path).map_err(|e| {
            format!(
                "Failed to read packet reviews '{}': {e}",
                reviews_path.display()
            )
        })?;
        serde_json::from_str(&reviews_data).map_err(|e| {
            format!(
                "Failed to parse packet reviews '{}': {e}",
                reviews_path.display()
            )
        })?
    } else {
        Vec::new()
    };
    let confidence_updates_path = dir.join("reviews/confidence-updates.json");
    let confidence_updates: Vec<ConfidenceUpdate> = if confidence_updates_path.is_file() {
        let updates_data = std::fs::read_to_string(&confidence_updates_path).map_err(|e| {
            format!(
                "Failed to read packet confidence updates '{}': {e}",
                confidence_updates_path.display()
            )
        })?;
        serde_json::from_str(&updates_data).map_err(|e| {
            format!(
                "Failed to parse packet confidence updates '{}': {e}",
                confidence_updates_path.display()
            )
        })?
    } else {
        Vec::new()
    };
    let events_path = dir.join("events/events.json");
    let events: Vec<StateEvent> = if events_path.is_file() {
        let events_data = std::fs::read_to_string(&events_path).map_err(|e| {
            format!(
                "Failed to read packet events '{}': {e}",
                events_path.display()
            )
        })?;
        serde_json::from_str(&events_data).map_err(|e| {
            format!(
                "Failed to parse packet events '{}': {e}",
                events_path.display()
            )
        })?
    } else {
        Vec::new()
    };
    let proposals_path = dir.join("proposals/proposals.json");
    let proposals: Vec<StateProposal> = if proposals_path.is_file() {
        let proposals_data = std::fs::read_to_string(&proposals_path).map_err(|e| {
            format!(
                "Failed to read packet proposals '{}': {e}",
                proposals_path.display()
            )
        })?;
        serde_json::from_str(&proposals_data).map_err(|e| {
            format!(
                "Failed to parse packet proposals '{}': {e}",
                proposals_path.display()
            )
        })?
    } else {
        Vec::new()
    };

    let overview_path = dir.join("overview.json");
    let overview: PacketOverviewHeader = if overview_path.is_file() {
        let overview_data = std::fs::read_to_string(&overview_path).map_err(|e| {
            format!(
                "Failed to read packet overview '{}': {e}",
                overview_path.display()
            )
        })?;
        serde_json::from_str(&overview_data).map_err(|e| {
            format!(
                "Failed to parse packet overview '{}': {e}",
                overview_path.display()
            )
        })?
    } else {
        PacketOverviewHeader::default()
    };

    let source = manifest.source.unwrap_or_default();
    let name = first_non_empty([
        source.project_name.as_str(),
        overview.project_name.as_str(),
        dir.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("packet"),
    ]);
    let description = first_non_empty([
        source.description.as_str(),
        overview.description.as_str(),
        "",
    ]);
    let compiled_at = first_non_empty([
        source.compiled_at.as_str(),
        overview.compiled_at.as_str(),
        "",
    ]);

    let mut project = project::assemble(name, findings, overview.papers_processed, 0, description);
    if !compiled_at.is_empty() {
        project.project.compiled_at = compiled_at.to_string();
    }
    if !source.compiler.is_empty() {
        project.project.compiler = source.compiler;
    }
    if !source.vela_version.is_empty() {
        project.vela_version = source.vela_version;
    }
    if !source.schema.is_empty() {
        project.schema = source.schema;
    }
    project.review_events = review_events;
    project.confidence_updates = confidence_updates;
    project.events = events;
    project.proposals = proposals;
    project::recompute_stats(&mut project);
    Ok(project)
}

fn load_vela_repo(dir: &Path) -> Result<Project, String> {
    let vela_dir = dir.join(".vela");
    let config_path = vela_dir.join("config.toml");
    let repository_profile = crate::frontier_repo::read_repository_profile(dir)?;

    // Profile v1 deliberately does not read reducer seed metadata from the
    // legacy mixed config. Migration removes that file; ignoring it here keeps
    // profile loading from accidentally restoring identity or dependency
    // authority from legacy TOML bytes.
    let profile_is_v1 = matches!(
        &repository_profile,
        Some(crate::frontier_repo::FrontierProfileFile::V1(_))
    );
    let config: RepoConfig = if profile_is_v1 {
        RepoConfig {
            project: RepoProjectMeta {
                name: dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                frontier_id: None,
                compiled_at: String::new(),
                description: String::new(),
                compiler: default_compiler(),
                papers_processed: 0,
            },
        }
    } else if config_path.exists() {
        let toml_str = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config.toml: {e}"))?;
        toml::from_str(&toml_str).map_err(|e| format!("Failed to parse config.toml: {e}"))?
    } else {
        RepoConfig {
            project: RepoProjectMeta {
                name: dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                frontier_id: None,
                compiled_at: String::new(),
                description: String::new(),
                compiler: default_compiler(),
                papers_processed: 0,
            },
        }
    };

    // Read findings
    let findings_dir = dir.join(".vela/findings");
    let mut findings: Vec<FindingBundle> = Vec::new();

    if findings_dir.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&findings_dir)
            .map_err(|e| format!("Failed to read findings/: {e}"))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort();

        for path in entries {
            let data = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let finding: FindingBundle = serde_json::from_str(&data)
                .map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;
            findings.push(finding);
        }
    }

    // Read reviews
    let reviews_dir = dir.join(".vela/reviews");
    let mut review_events: Vec<ReviewEvent> = Vec::new();
    if reviews_dir.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&reviews_dir)
            .map_err(|e| format!("Failed to read reviews/: {e}"))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort();

        for path in entries {
            let data = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let event: ReviewEvent = serde_json::from_str(&data)
                .map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;
            review_events.push(event);
        }
    }

    let confidence_updates_dir = dir.join(".vela/confidence-updates");
    let mut confidence_updates: Vec<ConfidenceUpdate> = Vec::new();
    if confidence_updates_dir.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&confidence_updates_dir)
            .map_err(|e| format!("Failed to read confidence-updates/: {e}"))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort();

        for path in entries {
            let data = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let update: ConfidenceUpdate = serde_json::from_str(&data)
                .map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;
            confidence_updates.push(update);
        }
    }
    let events_dir = dir.join(".vela/events");
    let proposals_dir = dir.join(".vela/proposals");
    let proof_state_path = vela_dir.join("proof-state.json");
    let mut events: Vec<StateEvent> = Vec::new();
    if events_dir.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&events_dir)
            .map_err(|e| format!("Failed to read events/: {e}"))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort();

        for path in entries {
            let data = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let event: StateEvent = serde_json::from_str(&data)
                .map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;
            events.push(event);
        }
    }
    let mut proposals: Vec<StateProposal> = Vec::new();
    if proposals_dir.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&proposals_dir)
            .map_err(|e| format!("Failed to read proposals/: {e}"))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort();

        for path in entries {
            let data = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let proposal: StateProposal = serde_json::from_str(&data)
                .map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;
            proposals.push(proposal);
        }
    }
    let proof_state = if proof_state_path.is_file() {
        let data = std::fs::read_to_string(&proof_state_path)
            .map_err(|e| format!("Failed to read {}: {e}", proof_state_path.display()))?;
        serde_json::from_str::<ProofState>(&data)
            .map_err(|e| format!("Failed to parse {}: {e}", proof_state_path.display()))?
    } else {
        ProofState::default()
    };

    let artifacts_dir = dir.join(".vela/artifacts");
    let mut artifacts: Vec<crate::bundle::Artifact> = Vec::new();
    if artifacts_dir.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&artifacts_dir)
            .map_err(|e| format!("Failed to read artifacts/: {e}"))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort();
        for path in entries {
            let data = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let artifact: crate::bundle::Artifact = serde_json::from_str(&data)
                .map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;
            artifacts.push(artifact);
        }
    }

    // Actor registry. Stored as one flat JSON file because actors are a
    // small authority list, not content-addressed frontier objects.
    let actors_path = dir.join(".vela/actors.json");
    let actors: Vec<crate::sign::ActorRecord> = if actors_path.is_file() {
        let data = std::fs::read_to_string(&actors_path)
            .map_err(|e| format!("Failed to read {}: {e}", actors_path.display()))?;
        serde_json::from_str(&data)
            .map_err(|e| format!("Failed to parse {}: {e}", actors_path.display()))?
    } else {
        Vec::new()
    };

    let signatures_path = dir.join(".vela/signatures.json");
    let signatures: Vec<crate::sign::SignedEnvelope> = if signatures_path.is_file() {
        let data = std::fs::read_to_string(&signatures_path)
            .map_err(|e| format!("Failed to read {}: {e}", signatures_path.display()))?;
        serde_json::from_str(&data)
            .map_err(|e| format!("Failed to parse {}: {e}", signatures_path.display()))?
    } else {
        Vec::new()
    };

    // V0.1 keeps its exact manifest/config behavior. Profile v1 contributes
    // display metadata only; identity and dependency state come exclusively
    // from the validated canonical identity-event set.
    let (display_name, display_description, manifest_deps, configured_frontier_id) =
        match repository_profile.as_ref() {
            Some(crate::frontier_repo::FrontierProfileFile::LegacyV0_1(manifest)) => (
                manifest.name.clone(),
                manifest.description.clone(),
                manifest.dependencies.frontiers_v2.clone(),
                manifest
                    .frontier_id
                    .clone()
                    .or_else(|| config.project.frontier_id.clone()),
            ),
            Some(crate::frontier_repo::FrontierProfileFile::V1(profile)) => {
                let authority =
                    crate::frontier_profile::EffectiveFrontierAuthorityV1::from_events(&events)?;
                profile.assert_frontier_id(&authority.frontier_id)?;
                (
                    profile.name.clone(),
                    profile.summary.clone(),
                    Vec::new(),
                    Some(authority.frontier_id),
                )
            }
            None => (
                config.project.name.clone(),
                config.project.description.clone(),
                Vec::new(),
                config.project.frontier_id.clone(),
            ),
        };
    let mut c = project::assemble(
        &display_name,
        findings,
        config.project.papers_processed,
        0,
        &display_description,
    );
    if profile_is_v1 {
        // Avoid the wall-clock timestamp produced by `assemble`; Profile v1
        // has no config seed. This value is display metadata and is excluded
        // from scientific_state_root_v2.
        c.project.compiled_at = events
            .iter()
            .map(|event| event.timestamp.as_str())
            .min()
            .unwrap_or("")
            .to_string();
    } else if !config.project.compiled_at.is_empty() {
        c.project.compiled_at = config.project.compiled_at;
    }
    c.project.compiler = config.project.compiler;
    if !manifest_deps.is_empty() {
        c.project.dependencies = manifest_deps;
    }
    c.review_events = review_events;
    c.confidence_updates = confidence_updates;
    c.events = events;
    c.frontier_id = configured_frontier_id.or_else(|| project::frontier_id_from_genesis(&c.events));
    c.proposals = proposals;
    c.proof_state = proof_state;
    c.actors = actors;
    c.signatures = signatures;
    c.artifacts = artifacts;

    // The loader IS the reducer. Every event-derived collection is
    // grafted from one full replay of the canonical log through
    // `reducer::apply_event` — there is no second, hand-maintained
    // dispatch table to forget. (The per-field `materialize_*_from_events`
    // helpers this replaces caused the same silent-drop bug four separate
    // times: trajectories at v0.55, diff-packs/verdict-conflicts at
    // v0.221, verifier attachments after the v0.700 cut, and attempts/
    // transfers/endorsements/contradictions — which had reducer arms but
    // NO loader path at all, so they vanished on every load.)
    //
    // `findings/` stays the assembly-order cache: cached findings feed
    // `snapshot_hash` and the locks byte-stably; the replayed findings
    // are the verification copy (`reducer::verify_replay`). A replay
    // failure degrades to the cache-only load with empty side tables —
    // a broken log must stay loadable for repair; `vela check` surfaces
    // the failure.
    match reducer::replayed_projection(&c) {
        Ok(replayed) => {
            c.released_diff_packs = replayed.released_diff_packs;
            c.verdict_conflicts = replayed.verdict_conflicts;
            c.verifier_attachments = replayed.verifier_attachments;
            c.contradictions = replayed.contradictions;
            c.attempts = replayed.attempts;
            c.attempt_resolutions = replayed.attempt_resolutions;
            c.transfers = replayed.transfers;
            c.endorsements = replayed.endorsements;
            c.statement_attestations = replayed.statement_attestations;
            c.anchor_links = replayed.anchor_links;
            c.attempt_claims = replayed.attempt_claims;
            c.statement_registrations = replayed.statement_registrations;
            // Locator repairs land on the replayed atoms (the reducer
            // arm); copy them onto the cache-derived atoms by id so the
            // curation work the canonical events recorded survives load.
            for atom in &mut c.evidence_atoms {
                if atom.locator.is_none()
                    && let Some(rep) = replayed
                        .evidence_atoms
                        .iter()
                        .find(|r| r.id == atom.id && r.locator.is_some())
                {
                    atom.locator = rep.locator.clone();
                    atom.caveats.retain(|cv| cv != "missing evidence locator");
                }
            }
        }
        Err(e) => {
            eprintln!(
                "warn · event-log replay failed during load (side tables empty until repaired): {e}"
            );
        }
    }

    // Invalid producer withdrawals never gain terminal standing. Keep their
    // immutable bytes visible for diagnostics, but project the affected
    // proposal as pending in ordinary reads; strict checks separately block
    // on the exact Receipt/signature failure.
    let invalid_withdrawal_proposals = c
        .events
        .iter()
        .filter(|event| event.kind == crate::events::EVENT_KIND_PROPOSAL_WITHDRAWN)
        .filter(|event| crate::proposals::verify_proposal_withdrawal_event(dir, &c, event).is_err())
        .map(|event| event.target.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    for proposal in &mut c.proposals {
        if invalid_withdrawal_proposals.contains(&proposal.id) {
            proposal.status = "pending_review".to_string();
            proposal.reviewed_by = None;
            proposal.reviewed_at = None;
            proposal.decision_reason = None;
            proposal.applied_event_id = None;
        }
    }

    project::recompute_stats(&mut c);

    Ok(c)
}

// ── Save ─────────────────────────────────────────────────────────────

/// Save a project to a detected source.
pub fn save(source: &VelaSource, project: &Project) -> Result<(), String> {
    match source {
        VelaSource::ProjectFile(path) => save_project_file(path, project),
        VelaSource::VelaRepo(dir) => save_vela_repo(dir, project),
        VelaSource::PacketDir(dir) => Err(format!(
            "Cannot save directly into packet directory '{}'. Export a new packet instead.",
            dir.display()
        )),
    }
}

fn save_project_file(path: &Path, project: &Project) -> Result<(), String> {
    let json = serde_json::to_string_pretty(project)
        .map_err(|e| format!("Failed to serialize project: {e}"))?;
    std::fs::write(path, json)
        .map_err(|e| format!("Failed to write project file '{}': {e}", path.display()))
}

/// Exact generated-file postimages for a split Vela repository.
///
/// `writes` is keyed by normalized frontier-relative path. `deletes` contains
/// only stale files in Vela-owned collections; user-owned and unknown files are
/// never inferred as deletions. Rendering is read-only so a transaction can
/// bind every preimage before installing any state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManagedFileSet {
    pub writes: BTreeMap<String, Vec<u8>>,
    pub deletes: BTreeSet<String>,
}

impl ManagedFileSet {
    fn insert(&mut self, path: String, bytes: Vec<u8>) -> Result<(), String> {
        if self.writes.insert(path.clone(), bytes).is_some() {
            return Err(format!("duplicate generated Vela path: {path}"));
        }
        self.deletes.remove(&path);
        Ok(())
    }
}

/// Render all files owned by `save_vela_repo`, including visible derived
/// views, without mutating `dir`.
pub fn render_vela_repo_files(dir: &Path, project: &Project) -> Result<ManagedFileSet, String> {
    let mut managed = ManagedFileSet::default();
    let profile_is_v1 = matches!(
        crate::frontier_repo::read_repository_profile(dir)?,
        Some(crate::frontier_repo::FrontierProfileFile::V1(_))
    );
    if !profile_is_v1 {
        let config = RepoConfig {
            project: RepoProjectMeta {
                name: project.project.name.clone(),
                frontier_id: Some(project.frontier_id()),
                compiled_at: project.project.compiled_at.clone(),
                description: project.project.description.clone(),
                compiler: project.project.compiler.clone(),
                papers_processed: project.project.papers_processed,
            },
        };
        managed.insert(
            ".vela/config.toml".to_string(),
            render_legacy_repo_config(&dir.join(".vela/config.toml"), &config)?,
        )?;
    }

    for finding in &project.findings {
        let path = managed_object_path("findings", &finding.id)?;
        managed.insert(
            path,
            serde_json::to_vec_pretty(finding)
                .map_err(|e| format!("Failed to serialize finding {}: {e}", finding.id))?,
        )?;
    }
    for event in &project.events {
        let path = managed_object_path("events", &event.id)?;
        let rendered = serde_json::to_vec_pretty(event)
            .map_err(|e| format!("Failed to serialize state event {}: {e}", event.id))?;
        let bytes = match std::fs::read(dir.join(&path)) {
            Ok(existing) => {
                let existing_event: StateEvent =
                    serde_json::from_slice(&existing).map_err(|error| {
                        format!(
                            "Existing immutable event {} cannot be decoded: {error}",
                            event.id
                        )
                    })?;
                let existing_value =
                    serde_json::to_value(&existing_event).map_err(|error| error.to_string())?;
                let candidate_value =
                    serde_json::to_value(event).map_err(|error| error.to_string())?;
                if existing_value == candidate_value {
                    existing
                } else {
                    rendered
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => rendered,
            Err(error) => {
                return Err(format!(
                    "Failed to read existing immutable event {}: {error}",
                    event.id
                ));
            }
        };
        managed.insert(path, bytes)?;
    }
    for proposal in &project.proposals {
        let path = managed_object_path("proposals", &proposal.id)?;
        managed.insert(
            path,
            serde_json::to_vec_pretty(proposal)
                .map_err(|e| format!("Failed to serialize proposal {}: {e}", proposal.id))?,
        )?;
    }
    for artifact in &project.artifacts {
        let path = managed_object_path("artifacts", &artifact.id)?;
        managed.insert(
            path,
            serde_json::to_vec_pretty(artifact)
                .map_err(|e| format!("Failed to serialize artifact {}: {e}", artifact.id))?,
        )?;
    }

    managed.insert(
        ".vela/proof-state.json".to_string(),
        serde_json::to_vec_pretty(&project.proof_state)
            .map_err(|e| format!("Failed to serialize proof state: {e}"))?,
    )?;
    managed.insert(
        ".vela/actors.json".to_string(),
        serde_json::to_vec_pretty(&project.actors)
            .map_err(|e| format!("Failed to serialize actors: {e}"))?,
    )?;
    if project.signatures.is_empty() {
        if dir.join(".vela/signatures.json").is_file() {
            managed.deletes.insert(".vela/signatures.json".to_string());
        }
    } else {
        managed.insert(
            ".vela/signatures.json".to_string(),
            serde_json::to_vec_pretty(&project.signatures)
                .map_err(|e| format!("Failed to serialize signatures: {e}"))?,
        )?;
    }

    for collection in ["findings", "events", "proposals", "artifacts"] {
        collect_stale_managed_objects(dir, collection, &managed.writes, &mut managed.deletes)?;
    }

    for (path, bytes) in crate::frontier_repo::render_visible_repo_files(dir, project)? {
        managed.insert(path, bytes)?;
    }
    Ok(managed)
}

fn managed_object_path(collection: &str, id: &str) -> Result<String, String> {
    if id.is_empty()
        || id == "."
        || id == ".."
        || id.contains('/')
        || id.contains('\\')
        || id.chars().any(char::is_control)
    {
        return Err(format!(
            "invalid {collection} id for split-repository storage: {id:?}"
        ));
    }
    Ok(format!(".vela/{collection}/{id}.json"))
}

fn collect_stale_managed_objects(
    dir: &Path,
    collection: &str,
    desired: &BTreeMap<String, Vec<u8>>,
    deletes: &mut BTreeSet<String>,
) -> Result<(), String> {
    let collection_dir = dir.join(".vela").join(collection);
    if !collection_dir.is_dir() {
        return Ok(());
    }
    let entries = std::fs::read_dir(&collection_dir)
        .map_err(|e| format!("Failed to read {}: {e}", collection_dir.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|e| format!("Failed to read {} entry: {e}", collection_dir.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Failed to inspect {}: {e}", entry.path().display()))?;
        if !file_type.is_file() {
            continue;
        }
        let Some(filename) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !filename.ends_with(".json") {
            continue;
        }
        let relative = format!(".vela/{collection}/{filename}");
        if !desired.contains_key(&relative) {
            deletes.insert(relative);
        }
    }
    Ok(())
}

fn save_vela_repo(dir: &Path, project: &Project) -> Result<(), String> {
    let managed = render_vela_repo_files(dir, project)?;
    let vela_dir = dir.join(".vela");
    let findings_dir = vela_dir.join("findings");
    let events_dir = vela_dir.join("events");
    let proposals_dir = vela_dir.join("proposals");
    let tasks_dir = vela_dir.join("tasks");
    let workspaces_dir = vela_dir.join("workspaces");
    let source_inbox_dir = vela_dir.join("source-inbox");
    let artifacts_dir = vela_dir.join("artifacts");

    // Create directories
    for d in [
        &vela_dir,
        &findings_dir,
        &events_dir,
        &proposals_dir,
        &tasks_dir,
        &workspaces_dir,
        &source_inbox_dir,
        &artifacts_dir,
    ] {
        std::fs::create_dir_all(d)
            .map_err(|e| format!("Failed to create directory {}: {e}", d.display()))?;
    }

    for relative in &managed.deletes {
        let path = dir.join(relative);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Failed to remove stale {}: {error}",
                    path.display()
                ));
            }
        }
    }
    for (relative, bytes) in managed.writes {
        let path = dir.join(&relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {e}", parent.display()))?;
        }
        std::fs::write(&path, bytes)
            .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
    }

    Ok(())
}

// ── Convenience ──────────────────────────────────────────────────────

/// Detect source type from path, then load.
pub fn load_from_path(path: &Path) -> Result<Project, String> {
    let source = detect(path)?;
    load(&source)
}

fn is_packet_dir(path: &Path) -> bool {
    let manifest_path = path.join("manifest.json");
    if !manifest_path.is_file() {
        return false;
    }
    let Ok(data) = std::fs::read_to_string(&manifest_path) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_str::<PacketManifestHeader>(&data) else {
        return false;
    };
    manifest.packet_format == "vela.frontier-packet"
}

fn first_non_empty<'a>(values: impl IntoIterator<Item = &'a str>) -> &'a str {
    values
        .into_iter()
        .find(|value| !value.is_empty())
        .unwrap_or("")
}

/// Detect source type from path, then save.
pub fn save_to_path(path: &Path, project: &Project) -> Result<(), String> {
    let source = detect(path)?;
    save(&source, project)
}

/// Initialize a VelaRepo from a Project at the given directory.
/// Creates the minimum public `.vela/` layout and writes frontier state.
pub fn init_repo(dir: &Path, project: &Project) -> Result<(), String> {
    let vela_dir = dir.join(".vela");
    std::fs::create_dir_all(&vela_dir).map_err(|e| format!("Failed to create .vela/: {e}"))?;
    save_vela_repo(dir, project)
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::*;

    use tempfile::TempDir;

    use crate::test_support::{make_finding, make_project};

    #[test]
    fn managed_repo_render_is_read_only_byte_equivalent_and_enumerates_stale_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("managed-render");
        let original = make_project(
            "managed-render",
            vec![make_finding("vf_live", 0.8, "theoretical")],
        );
        init_repo(&dir, &original).unwrap();
        for relative in [
            ".vela/findings/vf_stale.json",
            ".vela/events/vev_stale.json",
            ".vela/proposals/vpr_stale.json",
            ".vela/artifacts/va_stale.json",
            ".vela/signatures.json",
        ] {
            std::fs::write(dir.join(relative), b"stale").unwrap();
        }
        let mut next = original;
        next.project.description = "new managed render".to_string();
        let before = std::fs::read(dir.join("frontier.json")).unwrap();

        let rendered = render_vela_repo_files(&dir, &next).unwrap();

        assert_eq!(std::fs::read(dir.join("frontier.json")).unwrap(), before);
        for relative in [
            ".vela/findings/vf_stale.json",
            ".vela/events/vev_stale.json",
            ".vela/proposals/vpr_stale.json",
            ".vela/artifacts/va_stale.json",
            ".vela/signatures.json",
        ] {
            assert!(
                rendered.deletes.contains(relative),
                "missing stale generated path {relative}"
            );
            assert!(dir.join(relative).exists(), "render mutated {relative}");
        }
        assert!(rendered.writes.contains_key(".vela/config.toml"));
        assert!(rendered.writes.contains_key(".vela/findings/vf_live.json"));
        assert!(rendered.writes.contains_key("frontier.json"));
        assert!(rendered.writes.contains_key("proof/latest.json"));

        save_vela_repo(&dir, &next).unwrap();
        for (relative, bytes) in &rendered.writes {
            assert_eq!(
                std::fs::read(dir.join(relative)).unwrap(),
                *bytes,
                "rendered bytes differ for {relative}"
            );
        }
        for relative in &rendered.deletes {
            assert!(
                !dir.join(relative).exists(),
                "stale path survived {relative}"
            );
        }
    }

    #[test]
    fn managed_repo_render_preserves_unchanged_event_bytes() {
        use crate::events::{
            EVENT_SCHEMA, NULL_HASH, StateActor, StateEvent, StateTarget, compute_event_id,
        };

        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("immutable-event-render");
        let mut original = make_project("immutable-event-render", Vec::new());
        let mut event = StateEvent {
            schema: EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: "research_trace.review".into(),
            target: StateTarget {
                r#type: "frontier".to_string(),
                id: original.frontier_id(),
            },
            actor: StateActor {
                r#type: "human".to_string(),
                id: "reviewer:legacy".to_string(),
            },
            timestamp: "2026-07-01T00:00:00Z".to_string(),
            reason: "Immutable legacy event.".to_string(),
            before_hash: NULL_HASH.to_string(),
            after_hash: NULL_HASH.to_string(),
            payload: serde_json::json!({"fixture": true}),
            caveats: Vec::new(),
            signature: None,
        };
        event.id = compute_event_id(&event);
        original.events.push(event.clone());
        init_repo(&dir, &original).unwrap();

        let event_path = dir.join(".vela/events").join(format!("{}.json", event.id));
        let mut explicit_null = serde_json::to_value(&event).unwrap();
        explicit_null["signature"] = serde_json::Value::Null;
        let raw = format!(
            "{}\n",
            serde_json::to_string_pretty(&explicit_null).unwrap()
        )
        .into_bytes();
        std::fs::write(&event_path, &raw).unwrap();

        let mut next: Project =
            serde_json::from_value(serde_json::to_value(&original).unwrap()).unwrap();
        next.project.description = "unrelated derived-view update".to_string();
        let rendered = render_vela_repo_files(&dir, &next).unwrap();
        assert_eq!(
            rendered
                .writes
                .get(&format!(".vela/events/{}.json", event.id))
                .unwrap(),
            &raw
        );

        next.events
            .iter_mut()
            .find(|candidate| candidate.id == event.id)
            .unwrap()
            .reason = "Attempted historical rewrite.".to_string();
        let rendered = render_vela_repo_files(&dir, &next).unwrap();
        assert_ne!(
            rendered
                .writes
                .get(&format!(".vela/events/{}.json", event.id))
                .unwrap(),
            &raw,
            "an explicitly changed event candidate must render its new bytes"
        );
    }

    /// attempts / transfers / endorsements / contradictions have reducer
    /// arms but NO directory storage: the event log is their only
    /// persistence. Before the replay loader, `load_vela_repo` had no path
    /// for them at all, so a deposited attempt vanished on the next load
    /// (the 4th instance of the forgot-a-materializer bug class). The
    /// replay loader makes the reducer the single load path; this test
    /// pins it. When a new Project collection gains a reducer arm, it must
    /// also be grafted in `load_vela_repo` — extend this test with it.
    #[test]
    fn evented_side_tables_survive_save_and_load() {
        use crate::attempt::{Attempt, AttemptDraft};
        use crate::endorsement::{Endorsement, EndorsementDraft};
        use crate::events::{StateActor, StateEvent, StateTarget};
        use ed25519_dalek::SigningKey;
        use serde_json::json;

        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("side-tables");
        let mut original = make_project("side-tables", vec![]);
        let key = SigningKey::from_bytes(&[7u8; 32]);

        let attempt = Attempt::build(
            AttemptDraft {
                problem: 1,
                frontier: "f".to_string(),
                kind: "construction".into(),
                claim: "a(8) >= 33".to_string(),
                claimed_status: "banked".to_string(),
                ..Default::default()
            },
            &key,
        )
        .unwrap();
        let endorsement = Endorsement::build(
            EndorsementDraft {
                target_record: attempt.attempt_id.clone(),
                endorser: "reviewer:side-table-test".to_string(),
                dimension: String::new(),
                rationale: "pins the side-table survival contract".to_string(),
                at: "2026-01-01T00:00:00+00:00".to_string(),
            },
            &key,
        )
        .unwrap();

        let mk_event = |kind: &str,
                        target_type: &str,
                        target_id: &str,
                        payload: serde_json::Value,
                        ts: &str| StateEvent {
            schema: crate::events::EVENT_SCHEMA.to_string(),
            id: format!("vev_sidetable_{kind}").replace('.', "_"),
            kind: kind.into(),
            target: StateTarget {
                r#type: target_type.to_string(),
                id: target_id.to_string(),
            },
            actor: StateActor {
                id: "reviewer:side-table-test".to_string(),
                r#type: "human".to_string(),
            },
            timestamp: ts.to_string(),
            reason: "side-table survival test".to_string(),
            before_hash: "sha256:null".to_string(),
            after_hash: "sha256:null".to_string(),
            payload,
            caveats: vec![],
            signature: None,
        };
        original.events.push(mk_event(
            "attempt.deposited",
            "attempt",
            &attempt.attempt_id,
            json!({ "attempt": attempt }),
            "2026-01-01T00:00:01.000000+00:00",
        ));
        original.events.push(mk_event(
            "endorsement.deposited",
            "endorsement",
            &endorsement.endorsement_id,
            json!({ "endorsement": endorsement }),
            "2026-01-01T00:00:02.000000+00:00",
        ));

        init_repo(&dir, &original).unwrap();
        let loaded = load(&VelaSource::VelaRepo(dir)).unwrap();
        assert_eq!(
            loaded.attempts.len(),
            1,
            "attempt.deposited must survive save -> load (event log is its only storage)"
        );
        assert_eq!(loaded.attempts[0].attempt_id, attempt.attempt_id);
        assert_eq!(
            loaded.endorsements.len(),
            1,
            "endorsement.deposited must survive save -> load"
        );
        assert_eq!(
            loaded.endorsements[0].endorsement_id,
            endorsement.endorsement_id
        );
    }

    // ── regression: verifier attachments survive load/materialize ────
    // A `verifier_attachment.added` event must fold into
    // `Project.verifier_attachments` on load, exactly like the reducer arm
    // does on replay. Before the fix, `load_vela_repo` materialized every
    // other field individually but never this one, so `vela attach` evidence
    // vanished on the next `frontier materialize` and the trust gate could
    // never derive `verified` from persisted state.
    #[test]
    fn verifier_attachment_events_materialize_on_load() {
        use crate::verifier_attachment::{
            AdversarialProbe, AttachmentDraft, AttachmentOutcome, MatchToClaim, ProbeKind,
            ProbeResult, VerifierAttachment, VerifierMethod,
        };
        use serde_json::json;

        let att = VerifierAttachment::build(AttachmentDraft {
            target: "vf_0000000000000000".to_string(),
            claim_digest: "deadbeefdeadbeef".to_string(),
            verifier_method: VerifierMethod::ExactArithmeticRecompute,
            solver_id: "test-solver".to_string(),
            independent_of: vec![],
            match_to_claim: MatchToClaim {
                matches: true,
                checker_actor: "reviewer:test".to_string(),
            },
            adversarial_probes: vec![AdversarialProbe {
                kind: ProbeKind::CounterexampleSearch,
                result: ProbeResult::Survived,
                note: String::new(),
                evidence_root: String::new(),
            }],
            outcome: AttachmentOutcome::Passed,
            verifier_actor: "reviewer:test".to_string(),
            note: String::new(),
        })
        .expect("build attachment");

        let ev: crate::events::StateEvent = serde_json::from_value(json!({
            "schema": "vela.state_event.v0.1",
            "id": "vev_test000000000000",
            "kind": "verifier_attachment.added",
            "target": {"type": "finding", "id": "vf_0000000000000000"},
            "actor": {"id": "reviewer:test", "type": "reviewer"},
            "timestamp": "2026-01-01T00:00:00Z",
            "reason": "test",
            "before_hash": "",
            "after_hash": "",
            "payload": {"attachment": att},
        }))
        .expect("deserialize state event");

        let mut p = make_project("t", vec![]);
        p.events.push(ev);
        assert!(p.verifier_attachments.is_empty());
        // The replay loader is now the single fold path (loader = reducer).
        let replayed = reducer::replayed_projection(&p).expect("replay");
        assert_eq!(
            replayed.verifier_attachments.len(),
            1,
            "verifier_attachment.added must fold into verifier_attachments on replay"
        );
        assert_eq!(replayed.verifier_attachments[0].id, att.id);
        // idempotent under a duplicated event
        let dup = p.events[0].clone();
        p.events.push(dup);
        let replayed_twice = reducer::replayed_projection(&p).expect("replay twice");
        assert_eq!(replayed_twice.verifier_attachments.len(), 1);
    }

    // ── detect tests ────────────────────────────────────────────────

    #[test]
    fn detect_json_file() {
        let tmp = TempDir::new().unwrap();
        let json_path = tmp.path().join("test.json");
        std::fs::write(&json_path, "{}").unwrap();
        let source = detect(&json_path).unwrap();
        assert_eq!(source, VelaSource::ProjectFile(json_path));
    }

    #[test]
    fn detect_vela_repo() {
        let tmp = TempDir::new().unwrap();
        let repo_dir = tmp.path().join("my-repo");
        std::fs::create_dir_all(repo_dir.join(".vela")).unwrap();
        let source = detect(&repo_dir).unwrap();
        assert_eq!(source, VelaSource::VelaRepo(repo_dir));
    }

    #[test]
    fn detect_dir_without_vela_errors() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("plain-dir");
        std::fs::create_dir_all(&dir).unwrap();
        let result = detect(&dir);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("frontier packet"));
        assert!(error.contains("vela init"));
    }

    #[test]
    fn detect_nonexistent_json_path() {
        let path = Path::new("/tmp/nonexistent_test_vela.json");
        let source = detect(path).unwrap();
        assert_eq!(source, VelaSource::ProjectFile(path.to_path_buf()));
    }

    #[test]
    fn detect_nonexistent_non_json_errors() {
        let path = Path::new("/tmp/nonexistent_test_vela_dir");
        let result = detect(path);
        assert!(result.is_err());
    }

    // ── roundtrip: project file ────────────────────────────────────

    #[test]
    fn roundtrip_project_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");

        let mut f1 = make_finding("vf_001", 0.8, "mechanism");
        f1.add_link("vf_002", "extends", "shared entity");
        let f2 = make_finding("vf_002", 0.6, "therapeutic");
        let original = make_project("roundtrip-test", vec![f1, f2]);

        let source = VelaSource::ProjectFile(path.clone());
        save(&source, &original).unwrap();
        let loaded = load(&source).unwrap();

        assert_eq!(loaded.findings.len(), 2);
        assert_eq!(loaded.project.name, "roundtrip-test");
        assert_eq!(loaded.findings[0].links.len(), 1);
        assert_eq!(loaded.findings[0].links[0].target, "vf_002");
    }

    // ── roundtrip: vela repo ────────────────────────────────────────

    #[test]
    fn roundtrip_vela_repo() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("test-repo");

        let mut f1 = make_finding("vf_aaa", 0.9, "mechanism");
        f1.add_link("vf_bbb", "contradicts", "opposite direction");
        f1.add_link("vf_ccc", "supports", "same pathway");
        let f2 = make_finding("vf_bbb", 0.7, "therapeutic");
        let f3 = make_finding("vf_ccc", 0.5, "biomarker");
        let original = make_project("repo-test", vec![f1, f2, f3]);

        init_repo(&dir, &original).unwrap();

        // Verify directory structure
        assert!(dir.join(".vela").is_dir());
        assert!(dir.join(".vela/config.toml").exists());
        assert!(dir.join(".vela/findings").is_dir());
        assert!(dir.join(".vela/findings/vf_aaa.json").exists());
        assert!(dir.join(".vela/findings/vf_bbb.json").exists());
        assert!(dir.join(".vela/findings/vf_ccc.json").exists());
        assert!(dir.join(".vela/events").is_dir());
        assert!(dir.join(".vela/proposals").is_dir());
        assert!(dir.join(".vela/proof-state.json").exists());
        assert!(!dir.join(".vela/links/manifest.json").exists());
        assert!(!dir.join(".vela/reviews").exists());

        // Load back
        let source = VelaSource::VelaRepo(dir);
        let loaded = load(&source).unwrap();

        assert_eq!(loaded.findings.len(), 3);
        assert_eq!(loaded.project.name, "repo-test");
        assert_eq!(loaded.project.description, "Test project");

        // Check links redistributed correctly
        let f1_loaded = loaded.findings.iter().find(|f| f.id == "vf_aaa").unwrap();
        assert_eq!(f1_loaded.links.len(), 2);
        let f2_loaded = loaded.findings.iter().find(|f| f.id == "vf_bbb").unwrap();
        assert!(f2_loaded.links.is_empty());
    }

    // ── links remain embedded in finding bundles ─────────────────────

    #[test]
    fn embedded_links_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("link-test");

        let mut f1 = make_finding("vf_x1", 0.8, "mechanism");
        f1.add_link("vf_x2", "extends", "entity overlap");
        f1.add_link_with_source("vf_x3", "supports", "pathway link", "llm");
        let mut f2 = make_finding("vf_x2", 0.7, "mechanism");
        f2.add_link("vf_x1", "contradicts", "opposite");
        let f3 = make_finding("vf_x3", 0.6, "therapeutic");

        let original = make_project("link-test", vec![f1, f2, f3]);
        init_repo(&dir, &original).unwrap();

        assert!(!dir.join(".vela/links/manifest.json").exists());

        // Load back and verify redistribution
        let loaded = load(&VelaSource::VelaRepo(dir)).unwrap();
        let lf1 = loaded.findings.iter().find(|f| f.id == "vf_x1").unwrap();
        assert_eq!(lf1.links.len(), 2);
        let lf2 = loaded.findings.iter().find(|f| f.id == "vf_x2").unwrap();
        assert_eq!(lf2.links.len(), 1);
        assert_eq!(lf2.links[0].link_type, "contradicts");
    }

    // ── config.toml parsing ─────────────────────────────────────────

    #[test]
    fn legacy_config_save_preserves_unchanged_bytes_and_operational_sections() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("config-byte-preservation");
        let project = make_project("config-byte-preservation", vec![]);
        init_repo(&dir, &project).unwrap();

        let config_path = dir.join(".vela/config.toml");
        let mut exact = std::fs::read(&config_path).unwrap();
        exact.extend_from_slice(
            br#"
# operator-owned formatting and settings must survive a state save
[publish]
git_push = "off"

[work]
lease_ttl_seconds = 7200

[mcp]
profile = "draft"
"#,
        );
        std::fs::write(&config_path, &exact).unwrap();

        let rendered = render_vela_repo_files(&dir, &project).unwrap();
        assert_eq!(rendered.writes[".vela/config.toml"], exact);
        save_vela_repo(&dir, &project).unwrap();
        assert_eq!(std::fs::read(config_path).unwrap(), exact);
    }

    #[test]
    fn legacy_config_save_structurally_merges_project_changes() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("config-structural-preservation");
        let mut project = make_project("config-structural-preservation", vec![]);
        init_repo(&dir, &project).unwrap();

        let config_path = dir.join(".vela/config.toml");
        let mut value: toml::Value = std::fs::read_to_string(&config_path)
            .unwrap()
            .parse()
            .unwrap();
        value
            .get_mut("project")
            .and_then(toml::Value::as_table_mut)
            .unwrap()
            .insert(
                "legacy_seed".to_string(),
                toml::Value::String("retain-me".to_string()),
            );
        let root = value.as_table_mut().unwrap();
        root.insert(
            "publish".to_string(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "git_push".to_string(),
                toml::Value::String("off".to_string()),
            )])),
        );
        root.insert(
            "work".to_string(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "lease_ttl_seconds".to_string(),
                toml::Value::Integer(3600),
            )])),
        );
        root.insert(
            "custom".to_string(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "nested".to_string(),
                toml::Value::Array(vec![
                    toml::Value::String("one".to_string()),
                    toml::Value::String("two".to_string()),
                ]),
            )])),
        );
        std::fs::write(&config_path, toml::to_string_pretty(&value).unwrap()).unwrap();

        project.project.description = "updated project metadata".to_string();
        let rendered = render_vela_repo_files(&dir, &project).unwrap();
        let merged: toml::Value = std::str::from_utf8(&rendered.writes[".vela/config.toml"])
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            merged["project"]["description"].as_str(),
            Some("updated project metadata")
        );
        assert_eq!(merged["project"]["legacy_seed"].as_str(), Some("retain-me"));
        assert_eq!(merged["publish"]["git_push"].as_str(), Some("off"));
        assert_eq!(merged["work"]["lease_ttl_seconds"].as_integer(), Some(3600));
        assert_eq!(merged["custom"]["nested"].as_array().unwrap().len(), 2);

        save_vela_repo(&dir, &project).unwrap();
        assert_eq!(
            std::fs::read(&config_path).unwrap(),
            rendered.writes[".vela/config.toml"]
        );
    }

    #[test]
    fn legacy_config_save_refuses_to_clobber_invalid_or_non_utf8_bytes() {
        for (case, invalid) in [
            ("invalid-toml", b"[project\nname = broken\n".as_slice()),
            ("non-utf8", b"[project]\nname = \"broken\xff\"\n".as_slice()),
        ] {
            let tmp = TempDir::new().unwrap();
            let dir = tmp.path().join(format!("config-{case}-preservation"));
            let project = make_project(&format!("config-{case}-preservation"), vec![]);
            init_repo(&dir, &project).unwrap();

            let config_path = dir.join(".vela/config.toml");
            std::fs::write(&config_path, invalid).unwrap();
            assert!(render_vela_repo_files(&dir, &project).is_err());
            assert!(save_vela_repo(&dir, &project).is_err());
            assert_eq!(std::fs::read(config_path).unwrap(), invalid);
        }
    }

    #[test]
    fn config_toml_parsing() {
        let toml_str = r#"
[project]
name = "alzheimers-tau"
description = "Tau pathology in Alzheimer's disease"
compiler = "vela/0.2.0"
papers_processed = 700
"#;
        let config: RepoConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.project.name, "alzheimers-tau");
        assert_eq!(
            config.project.description,
            "Tau pathology in Alzheimer's disease"
        );
        assert_eq!(config.project.papers_processed, 700);
        assert_eq!(config.project.compiler, "vela/0.2.0");
        assert_eq!(config.project.frontier_id, None);
        assert_eq!(config.project.compiled_at, "");
    }

    #[test]
    fn config_toml_minimal() {
        let toml_str = r#"
[project]
name = "minimal"
"#;
        let config: RepoConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.project.name, "minimal");
        assert_eq!(config.project.description, "");
        assert_eq!(config.project.papers_processed, 0);
    }

    #[test]
    fn vela_repo_persists_frontier_id_and_actors() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("actor-repo");

        let mut original = make_project(
            "actor-test",
            vec![make_finding("vf_actor", 0.8, "mechanism")],
        );
        let expected_frontier_id = original.frontier_id();
        original.actors.push(crate::sign::ActorRecord {
            id: "reviewer:test".into(),
            public_key: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            algorithm: "ed25519".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        original.signatures.push(crate::sign::SignedEnvelope {
            finding_id: "vf_actor".into(),
            signature: "00".repeat(64),
            public_key: "aa".repeat(32),
            signed_at: "2026-01-01T00:00:00Z".into(),
            algorithm: "ed25519".into(),
        });

        init_repo(&dir, &original).unwrap();
        assert!(dir.join(".vela/actors.json").exists());
        assert!(dir.join(".vela/signatures.json").exists());

        let first_load = load(&VelaSource::VelaRepo(dir.clone())).unwrap();
        let second_load = load(&VelaSource::VelaRepo(dir)).unwrap();

        assert_eq!(first_load.frontier_id(), expected_frontier_id);
        assert_eq!(second_load.frontier_id(), expected_frontier_id);
        assert_eq!(first_load.actors, original.actors);
        assert_eq!(first_load.signatures.len(), 1);
        assert_eq!(second_load.signatures.len(), 1);
        assert_eq!(second_load.signatures[0].finding_id, "vf_actor");
    }

    // ── empty project ──────────────────────────────────────────────

    #[test]
    fn empty_project_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("empty-repo");

        let original = make_project("empty", vec![]);
        init_repo(&dir, &original).unwrap();

        let loaded = load(&VelaSource::VelaRepo(dir)).unwrap();
        assert_eq!(loaded.findings.len(), 0);
        assert_eq!(loaded.stats.findings, 0);
        assert_eq!(loaded.stats.links, 0);
        assert_eq!(loaded.project.name, "empty");
    }

    #[test]
    fn artifacts_roundtrip_from_vela_repo() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("artifact-repo");

        let mut original = make_project("artifact-test", vec![]);
        let artifact = Artifact::new(
            "protocol",
            "trial protocol",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            Some(17),
            Some("application/json".into()),
            "local_blob",
            Some(".vela/artifact-blobs/sha256/bbbb".into()),
            Some("https://example.test/protocol".into()),
            Some("CC0-1.0".into()),
            vec!["vf_target".into()],
            Provenance {
                source_type: "clinical_trial".into(),
                doi: None,
                url: Some("https://example.test/protocol".into()),
                title: "trial protocol".into(),
                authors: vec![],
                year: Some(2026),
                license: Some("CC0-1.0".into()),
                publisher: None,
                funders: vec![],
                extraction: Extraction::default(),
                review: None,
                contributions: Vec::new(),
            },
            std::collections::BTreeMap::new(),
            crate::access_tier::AccessTier::Public,
        )
        .unwrap();
        let id = artifact.id.clone();
        original.artifacts.push(artifact);
        init_repo(&dir, &original).unwrap();

        let loaded = load(&VelaSource::VelaRepo(dir.clone())).unwrap();
        assert_eq!(loaded.artifacts.len(), 1);
        assert_eq!(loaded.artifacts[0].id, id);
        assert!(dir.join(".vela/artifacts").is_dir());
    }

    // ── large finding count ─────────────────────────────────────────

    #[test]
    fn large_finding_count() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("large-repo");

        let findings: Vec<FindingBundle> = (0..100)
            .map(|i| make_finding(&format!("vf_{i:04}"), 0.5 + (i as f64) * 0.004, "mechanism"))
            .collect();
        let original = make_project("large", findings);
        assert_eq!(original.findings.len(), 100);

        init_repo(&dir, &original).unwrap();

        let loaded = load(&VelaSource::VelaRepo(dir)).unwrap();
        assert_eq!(loaded.findings.len(), 100);
        assert_eq!(loaded.stats.findings, 100);
    }

    // ── legacy review events remain readable ─────────────────────────

    #[test]
    fn legacy_review_events_load() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("review-repo");

        let mut original =
            make_project("review-test", vec![make_finding("vf_r1", 0.8, "mechanism")]);
        original.review_events.push(ReviewEvent {
            id: "rev_001".into(),
            workspace: None,
            finding_id: "vf_r1".into(),
            reviewer: "0000-0001-2345-6789".into(),
            reviewed_at: "2024-01-01T00:00:00Z".into(),
            scope: None,
            status: None,
            action: ReviewAction::Approved,
            reason: "Looks correct".into(),
            evidence_considered: vec![],
            state_change: None,
        });

        init_repo(&dir, &original).unwrap();
        assert!(!dir.join(".vela/reviews").exists());
        std::fs::create_dir_all(dir.join(".vela/reviews")).unwrap();
        std::fs::write(
            dir.join(".vela/reviews/rev_001.json"),
            serde_json::to_string_pretty(&original.review_events[0]).unwrap(),
        )
        .unwrap();

        let loaded = load(&VelaSource::VelaRepo(dir)).unwrap();
        assert_eq!(loaded.review_events.len(), 1);
        assert_eq!(loaded.review_events[0].id, "rev_001");
        assert_eq!(loaded.review_events[0].finding_id, "vf_r1");
    }

    #[test]
    fn load_vela_repo_accepts_bbb_review_artifact() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("bbb-review-repo");
        std::fs::create_dir_all(dir.join(".vela/reviews")).unwrap();
        std::fs::write(
            dir.join(".vela/config.toml"),
            "[project]\nname = \"bbb-review-repo\"\ndescription = \"\"\ncompiler = \"vela/test\"\npapers_processed = 0\n",
        )
        .unwrap();
        std::fs::write(
            dir.join(".vela/reviews/rev_001_bbb_correction.json"),
            include_str!("../../embedded/tests/fixtures/legacy/rev_001_bbb_correction.json"),
        )
        .unwrap();

        let loaded = load(&VelaSource::VelaRepo(dir)).unwrap();
        assert_eq!(loaded.review_events.len(), 1);
        assert!(matches!(
            loaded.review_events[0].action,
            ReviewAction::Qualified { .. }
        ));
        assert_eq!(loaded.review_events[0].status.as_deref(), Some("accepted"));
    }

    // ── load_from_path convenience ──────────────────────────────────

    #[test]
    fn load_from_path_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("convenience.json");

        let original = make_project("convenience", vec![make_finding("vf_c1", 0.8, "mechanism")]);
        let json = serde_json::to_string_pretty(&original).unwrap();
        std::fs::write(&path, json).unwrap();

        let loaded = load_from_path(&path).unwrap();
        assert_eq!(loaded.project.name, "convenience");
        assert_eq!(loaded.findings.len(), 1);
    }

    #[test]
    fn load_from_path_repo() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("conv-repo");

        let original = make_project("conv-repo", vec![make_finding("vf_cr1", 0.8, "mechanism")]);
        init_repo(&dir, &original).unwrap();

        let loaded = load_from_path(&dir).unwrap();
        assert_eq!(loaded.project.name, "conv-repo");
        assert_eq!(loaded.findings.len(), 1);
    }

    // ── project file -> repo -> project file roundtrip ────────────

    #[test]
    fn full_format_roundtrip() {
        let tmp = TempDir::new().unwrap();

        // Create a project with findings and links
        let mut f1 = make_finding("vf_rt1", 0.85, "mechanism");
        f1.add_link("vf_rt2", "extends", "shared protein");
        let f2 = make_finding("vf_rt2", 0.72, "therapeutic");

        let original = make_project("full-roundtrip", vec![f1, f2]);

        // Save as JSON
        let json_path = tmp.path().join("original.json");
        save(&VelaSource::ProjectFile(json_path.clone()), &original).unwrap();

        // Load from JSON
        let from_json = load(&VelaSource::ProjectFile(json_path)).unwrap();

        // Save as repo
        let repo_dir = tmp.path().join("repo");
        init_repo(&repo_dir, &from_json).unwrap();

        // Load from repo
        let from_repo = load(&VelaSource::VelaRepo(repo_dir)).unwrap();

        // Verify structural equivalence
        assert_eq!(from_repo.findings.len(), from_json.findings.len());
        assert_eq!(from_repo.project.name, from_json.project.name);

        let rt1 = from_repo
            .findings
            .iter()
            .find(|f| f.id == "vf_rt1")
            .unwrap();
        assert_eq!(rt1.links.len(), 1);
        assert_eq!(rt1.links[0].target, "vf_rt2");
        assert_eq!(rt1.links[0].link_type, "extends");
    }
}
