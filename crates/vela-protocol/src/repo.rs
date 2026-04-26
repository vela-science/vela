//! Git-native VelaRepo abstraction — load/save projects from either monolithic JSON
//! or a `.vela/` directory of individual finding files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::bundle::{ConfidenceUpdate, FindingBundle, Link, ReviewEvent};
use crate::events::StateEvent;
use crate::project::{self, Project};
use crate::proposals::{ProofState, StateProposal};

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
            "Directory '{}' is not a Vela repository or frontier packet. Run `vela init`, `vela import`, or `vela migrate` first.",
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

#[derive(Debug, Serialize, Deserialize)]
struct RepoConfig {
    project: RepoProjectMeta,
}

#[derive(Debug, Serialize, Deserialize)]
struct RepoProjectMeta {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_compiler")]
    compiler: String,
    #[serde(default)]
    papers_processed: usize,
}

fn default_compiler() -> String {
    "vela/0.2.0".into()
}

// ── Link manifest ────────────────────────────────────────────────────

/// A link record in the centralized manifest. Contains a `source` field
/// (the finding ID that owns this link) so we can redistribute on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestLink {
    source: String,
    target: String,
    #[serde(rename = "type")]
    link_type: String,
    #[serde(default)]
    note: String,
    #[serde(default = "default_inferred_by")]
    inferred_by: String,
    #[serde(default)]
    created_at: String,
}

fn default_inferred_by() -> String {
    "compiler".into()
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

fn load_project_file(path: &Path) -> Result<Project, String> {
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

    // Read config
    let config: RepoConfig = if config_path.exists() {
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

    // Read link manifest and redistribute
    let links_dir = dir.join(".vela/links");
    let manifest_path = links_dir.join("manifest.json");
    if manifest_path.exists() {
        let data = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Failed to read links/manifest.json: {e}"))?;
        let manifest_links: Vec<ManifestLink> = serde_json::from_str(&data)
            .map_err(|e| format!("Failed to parse links/manifest.json: {e}"))?;

        // Build a map of source_id -> links
        let mut links_by_source: HashMap<String, Vec<Link>> = HashMap::new();
        for ml in manifest_links {
            links_by_source
                .entry(ml.source.clone())
                .or_default()
                .push(Link {
                    target: ml.target,
                    link_type: ml.link_type,
                    note: ml.note,
                    inferred_by: ml.inferred_by,
                    created_at: ml.created_at,
                });
        }

        // Distribute links into findings
        for finding in &mut findings {
            if let Some(links) = links_by_source.remove(&finding.id) {
                finding.links = links;
            }
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

    // Assemble into Project using the project::assemble function for stats,
    // then patch metadata from config.
    let mut c = project::assemble(
        &config.project.name,
        findings,
        config.project.papers_processed,
        0,
        &config.project.description,
    );
    c.project.compiler = config.project.compiler;
    c.review_events = review_events;
    c.confidence_updates = confidence_updates;
    c.events = events;
    c.proposals = proposals;
    c.proof_state = proof_state;
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

fn save_vela_repo(dir: &Path, project: &Project) -> Result<(), String> {
    let vela_dir = dir.join(".vela");
    let findings_dir = vela_dir.join("findings");
    let events_dir = vela_dir.join("events");
    let proposals_dir = vela_dir.join("proposals");

    // Create directories
    for d in [&vela_dir, &findings_dir, &events_dir, &proposals_dir] {
        std::fs::create_dir_all(d)
            .map_err(|e| format!("Failed to create directory {}: {e}", d.display()))?;
    }

    // Write config.toml
    let config = RepoConfig {
        project: RepoProjectMeta {
            name: project.project.name.clone(),
            description: project.project.description.clone(),
            compiler: project.project.compiler.clone(),
            papers_processed: project.project.papers_processed,
        },
    };
    let toml_str = toml::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config.toml: {e}"))?;
    std::fs::write(vela_dir.join("config.toml"), toml_str)
        .map_err(|e| format!("Failed to write config.toml: {e}"))?;

    // Write each finding as findings/{id}.json. Links remain embedded in the
    // finding bundle; legacy link manifests are still accepted on load.
    for finding in &project.findings {
        let json = serde_json::to_string_pretty(finding)
            .map_err(|e| format!("Failed to serialize finding {}: {e}", finding.id))?;
        let filename = format!("{}.json", finding.id);
        std::fs::write(findings_dir.join(&filename), json)
            .map_err(|e| format!("Failed to write {}: {e}", filename))?;
    }

    for event in &project.events {
        let json = serde_json::to_string_pretty(event)
            .map_err(|e| format!("Failed to serialize state event {}: {e}", event.id))?;
        let filename = format!("{}.json", event.id);
        std::fs::write(events_dir.join(&filename), json)
            .map_err(|e| format!("Failed to write event {}: {e}", filename))?;
    }

    for proposal in &project.proposals {
        let json = serde_json::to_string_pretty(proposal)
            .map_err(|e| format!("Failed to serialize proposal {}: {e}", proposal.id))?;
        let filename = format!("{}.json", proposal.id);
        std::fs::write(proposals_dir.join(&filename), json)
            .map_err(|e| format!("Failed to write proposal {}: {e}", filename))?;
    }

    let proof_state_json = serde_json::to_string_pretty(&project.proof_state)
        .map_err(|e| format!("Failed to serialize proof state: {e}"))?;
    std::fs::write(vela_dir.join("proof-state.json"), proof_state_json)
        .map_err(|e| format!("Failed to write proof-state.json: {e}"))?;

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
    use crate::project;
    use tempfile::TempDir;

    fn make_finding(id: &str, score: f64, assertion_type: &str) -> FindingBundle {
        FindingBundle {
            id: id.into(),
            version: 1,
            previous_version: None,
            assertion: Assertion {
                text: format!("Finding {id}"),
                assertion_type: assertion_type.into(),
                entities: vec![Entity {
                    name: "TestEntity".into(),
                    entity_type: "protein".into(),
                    identifiers: serde_json::Map::new(),
                    canonical_id: None,
                    candidates: vec![],
                    aliases: vec![],
                    resolution_provenance: None,
                    resolution_confidence: 1.0,
                    resolution_method: None,
                    species_context: None,
                    needs_review: false,
                }],
                relation: None,
                direction: None,
            },
            evidence: Evidence {
                evidence_type: "experimental".into(),
                model_system: String::new(),
                species: None,
                method: String::new(),
                sample_size: None,
                effect_size: None,
                p_value: None,
                replicated: false,
                replication_count: None,
                evidence_spans: vec![],
            },
            conditions: Conditions {
                text: String::new(),
                species_verified: vec![],
                species_unverified: vec![],
                in_vitro: false,
                in_vivo: false,
                human_data: false,
                clinical_trial: false,
                concentration_range: None,
                duration: None,
                age_group: None,
                cell_type: None,
            },
            confidence: Confidence::legacy(score, "seeded prior", 0.85),
            provenance: Provenance {
                source_type: "published_paper".into(),
                doi: None,
                pmid: None,
                pmc: None,
                openalex_id: None,
                url: None,
                title: "Test".into(),
                authors: vec![],
                year: Some(2024),
                journal: None,
                license: None,
                publisher: None,
                funders: vec![],
                extraction: Extraction::default(),
                review: None,
                citation_count: None,
            },
            flags: Flags {
                gap: false,
                negative_space: false,
                contested: false,
                retracted: false,
                declining: false,
                gravity_well: false,
                review_state: None,
                superseded: false,
            },
            links: vec![],
            annotations: vec![],
            attachments: vec![],
            created: String::new(),
            updated: None,
        }
    }

    fn make_project(name: &str, findings: Vec<FindingBundle>) -> Project {
        project::assemble(name, findings, 10, 0, "Test project")
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
            include_str!("../../../tests/fixtures/legacy/rev_001_bbb_correction.json"),
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

    #[test]
    fn load_from_path_packet_dir() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("packet-frontier");

        let mut original = make_project(
            "packet-frontier",
            vec![make_finding("vf_pkt1", 0.81, "mechanism")],
        );
        original.review_events.push(ReviewEvent {
            id: "rev_pkt1".into(),
            workspace: Some("bbb".into()),
            finding_id: "vf_pkt1".into(),
            reviewer: "reviewer:test".into(),
            reviewed_at: "2026-01-01T00:00:00Z".into(),
            scope: Some("external".into()),
            status: Some("accepted".into()),
            action: ReviewAction::Approved,
            reason: "Imported from another lab".into(),
            evidence_considered: vec![],
            state_change: None,
        });
        original.stats.review_event_count = original.review_events.len();
        crate::export::export_packet(&original, &dir).unwrap();

        let loaded = load_from_path(&dir).unwrap();
        assert_eq!(loaded.project.name, "packet-frontier");
        assert_eq!(loaded.findings.len(), 1);
        assert_eq!(loaded.review_events.len(), 1);
        assert_eq!(loaded.stats.review_event_count, 1);
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
