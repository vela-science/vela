//! Export frontier findings in proof-first formats: CSV, JSON-LD, BibTeX, Markdown, Frontier JSON, and Packet.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::bundle::FindingBundle;
use crate::project::Project;
use crate::{events, packet, repo, signals, sources, state};

/// Supported export formats.
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Csv,
    JsonLd,
    BibTex,
    Markdown,
    /// Export as monolithic frontier JSON (useful for converting VelaRepo back to JSON).
    Project,
    /// Export a bounded proof packet as a directory.
    Packet,
}

impl ExportFormat {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "csv" => Ok(Self::Csv),
            "jsonld" | "json-ld" => Ok(Self::JsonLd),
            "bibtex" | "bib" => Ok(Self::BibTex),
            "markdown" | "md" => Ok(Self::Markdown),
            "frontier" | "json" => Ok(Self::Project),
            "packet" => Ok(Self::Packet),
            _ => Err(format!(
                "Unknown format '{}'. Supported: csv, jsonld, bibtex, markdown, frontier, packet",
                s
            )),
        }
    }

    /// Returns true if this format produces multiple files (a directory).
    pub fn is_multi_file(&self) -> bool {
        matches!(self, Self::Packet)
    }
}

pub fn export(frontier: &Project, format: ExportFormat) -> String {
    match format {
        ExportFormat::Csv => export_csv(frontier),
        ExportFormat::JsonLd => export_jsonld(frontier),
        ExportFormat::BibTex => export_bibtex(frontier),
        ExportFormat::Markdown => export_markdown(frontier),
        ExportFormat::Project => {
            serde_json::to_string_pretty(frontier).expect("Failed to serialize frontier")
        }
        ExportFormat::Packet => {
            panic!("Packet format is multi-file. Use export_packet() instead of export().");
        }
    }
}

pub fn run(frontier_path: &Path, format_str: &str, output: Option<&Path>) {
    let frontier = repo::load_from_path(frontier_path).expect("Failed to load frontier");

    let format = match ExportFormat::from_str(format_str) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{} {e}", crate::cli_style::err_prefix());
            std::process::exit(1);
        }
    };

    if format.is_multi_file() {
        let out_dir = output.unwrap_or_else(|| {
            eprintln!(
                "{} {} format requires --output <directory>",
                crate::cli_style::err_prefix(),
                format_str
            );
            std::process::exit(1);
        });
        let result = match format {
            ExportFormat::Packet => export_packet(&frontier, out_dir).map(|_| ()),
            _ => unreachable!("single-file format reached multi-file branch"),
        };
        match result {
            Ok(()) => {
                eprintln!(
                    "sealed · {} findings as {} · {}",
                    frontier.findings.len(),
                    format_str,
                    out_dir.display()
                );
            }
            Err(e) => {
                eprintln!("{} {e}", crate::cli_style::err_prefix());
                std::process::exit(1);
            }
        }
        return;
    }

    let result = export(&frontier, format);

    if let Some(out_path) = output {
        std::fs::write(out_path, &result).expect("Failed to write output file");
        eprintln!(
            "sealed · {} findings · {}",
            frontier.findings.len(),
            out_path.display()
        );
    } else {
        print!("{result}");
    }
}

// ── CSV ──────────────────────────────────────────────────────────────────────

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn export_csv(frontier: &Project) -> String {
    let mut out = String::new();
    out.push_str("id,assertion_type,assertion_text,confidence,replicated,entities,year,doi,source_title,gap,contested\n");

    for f in &frontier.findings {
        let entities: Vec<&str> = f
            .assertion
            .entities
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        let row = format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&f.id),
            csv_escape(&f.assertion.assertion_type),
            csv_escape(&f.assertion.text),
            f.confidence.score,
            f.evidence.replicated,
            csv_escape(&entities.join(";")),
            f.provenance.year.map(|y| y.to_string()).unwrap_or_default(),
            csv_escape(f.provenance.doi.as_deref().unwrap_or("")),
            csv_escape(&f.provenance.title),
            f.flags.gap,
            f.flags.contested,
        );
        out.push_str(&row);
    }
    out
}

// ── JSON-LD ──────────────────────────────────────────────────────────────────

fn export_jsonld(frontier: &Project) -> String {
    let items: Vec<serde_json::Value> = frontier
        .findings
        .iter()
        .map(|f| {
            // Build entity array with identifiers
            let entities: Vec<serde_json::Value> = f
                .assertion
                .entities
                .iter()
                .map(|e| {
                    let mut entity = serde_json::json!({
                        "vela:entityName": e.name,
                        "vela:entityType": e.entity_type,
                    });
                    // Add canonical identifier if resolved
                    if let Some(canonical) = &e.canonical_id {
                        let url = match canonical.source.as_str() {
                            "uniprot" => {
                                format!("https://www.uniprot.org/uniprot/{}", canonical.id)
                            }
                            "pubchem" => format!(
                                "https://pubchem.ncbi.nlm.nih.gov/compound/{}",
                                canonical.id
                            ),
                            "mesh" => format!("https://id.nlm.nih.gov/mesh/{}", canonical.id),
                            "ncbi_gene" => {
                                format!("https://www.ncbi.nlm.nih.gov/gene/{}", canonical.id)
                            }
                            "chebi" => format!(
                                "https://www.ebi.ac.uk/chebi/searchId.do?chebiId={}",
                                canonical.id
                            ),
                            "go" => {
                                format!("http://amigo.geneontology.org/amigo/term/{}", canonical.id)
                            }
                            _ => format!("urn:{}:{}", canonical.source, canonical.id),
                        };
                        entity["schema:identifier"] = serde_json::json!({"@id": url});
                    }
                    entity
                })
                .collect();

            // Build link array
            let links: Vec<serde_json::Value> = f
                .links
                .iter()
                .map(|l| {
                    serde_json::json!({
                        "vela:linkTarget": {"@id": format!("vela:{}", l.target)},
                        "vela:linkType": l.link_type,
                    })
                })
                .collect();

            // Build provenance activity
            let mut activity = serde_json::json!({
                "@type": "prov:Activity",
                "prov:wasAssociatedWith": format!("vela/{}", env!("CARGO_PKG_VERSION")),
            });
            if let Some(doi) = &f.provenance.doi {
                activity["prov:used"] = serde_json::json!({"@id": format!("doi:{doi}")});
            }

            let mut node = serde_json::json!({
                "@id": format!("vela:{}", f.id),
                "@type": "vela:FindingBundle",
                "vela:assertionText": f.assertion.text,
                "vela:assertionType": f.assertion.assertion_type,
                "vela:confidence": f.confidence.score,
                "vela:evidenceType": f.evidence.evidence_type,
                "schema:dateCreated": f.created,
                "prov:wasGeneratedBy": activity,
            });

            if !entities.is_empty() {
                node["vela:hasEntity"] = serde_json::Value::Array(entities);
            }
            if !links.is_empty() {
                node["vela:hasLink"] = serde_json::Value::Array(links);
            }

            node
        })
        .collect();

    let doc = serde_json::json!({
        "@context": {
            "@vocab": "https://vela.science/schema/",
            "schema": "https://schema.org/",
            "prov": "http://www.w3.org/ns/prov#",
            "np": "http://www.nanopub.org/nschema#",
            "doi": "https://doi.org/",
            "orcid": "https://orcid.org/"
        },
        "@graph": items,
    });

    serde_json::to_string_pretty(&doc).unwrap_or_default()
}

// ── BibTeX ───────────────────────────────────────────────────────────────────

fn export_bibtex(frontier: &Project) -> String {
    // Deduplicate by DOI (or title if no DOI).
    let mut seen: BTreeMap<String, &FindingBundle> = BTreeMap::new();
    for f in &frontier.findings {
        let key = f
            .provenance
            .doi
            .clone()
            .unwrap_or_else(|| f.provenance.title.clone());
        seen.entry(key).or_insert(f);
    }

    let mut out = String::new();
    for f in seen.values() {
        let cite_key = f
            .provenance
            .doi
            .as_deref()
            .map(|d| d.replace(['/', '.'], "_"))
            .unwrap_or_else(|| f.id.clone());

        let authors_str: String = f
            .provenance
            .authors
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(" and ");

        out.push_str(&format!("@article{{{},\n", cite_key));
        out.push_str(&format!("  title = {{{}}},\n", f.provenance.title));
        if !authors_str.is_empty() {
            out.push_str(&format!("  author = {{{}}},\n", authors_str));
        }
        if let Some(year) = f.provenance.year {
            out.push_str(&format!("  year = {{{}}},\n", year));
        }
        if let Some(journal) = &f.provenance.journal {
            out.push_str(&format!("  journal = {{{}}},\n", journal));
        }
        if let Some(doi) = &f.provenance.doi {
            out.push_str(&format!("  doi = {{{}}},\n", doi));
        }
        out.push_str("}\n\n");
    }
    out
}

#[derive(Debug, Clone, Serialize)]
struct PacketOverview {
    project_name: String,
    description: String,
    compiled_at: String,
    generated_at: String,
    findings: usize,
    papers_processed: usize,
    avg_confidence: f64,
    categories: BTreeMap<String, usize>,
    link_types: BTreeMap<String, usize>,
    top_entities: Vec<PacketEntitySummary>,
}

#[derive(Debug, Clone, Serialize)]
struct PacketEntitySummary {
    name: String,
    entity_type: String,
    finding_count: usize,
    categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PacketFindingSummary {
    id: String,
    assertion_type: String,
    assertion_text: String,
    confidence: f64,
    evidence_type: String,
    method: String,
    entities: Vec<String>,
    doi: Option<String>,
    source_title: String,
    flags: PacketFlags,
    link_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct PacketFlags {
    gap: bool,
    contested: bool,
    replicated: bool,
}

#[derive(Debug, Clone, Serialize)]
struct PacketBridgeSummary {
    entity: String,
    entity_type: String,
    categories: BTreeMap<String, usize>,
    finding_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PacketContradictionSummary {
    source_id: String,
    target_id: String,
    link_type: String,
    source_assertion: String,
    target_assertion: String,
}

#[derive(Debug, Clone, Serialize)]
struct PacketScope {
    frontier_name: String,
    description: String,
    generated_at: String,
    source_schema: String,
    finding_count: usize,
    papers_processed: usize,
    review_event_count: usize,
    intended_use: Vec<String>,
    out_of_scope: Vec<String>,
    caveats: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PacketSourceRow {
    source_key: String,
    source_id: String,
    locator: String,
    content_hash: Option<String>,
    title: String,
    doi: Option<String>,
    pmid: Option<String>,
    year: Option<i32>,
    source_type: String,
    extraction_mode: String,
    source_quality: String,
    caveats: Vec<String>,
    finding_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PacketEvidenceMatrixRow {
    finding_id: String,
    assertion_type: String,
    evidence_type: String,
    method: String,
    confidence: f64,
    replicated: bool,
    human_data: bool,
    clinical_trial: bool,
    source_key: String,
    source_id: Option<String>,
    evidence_atom_ids: Vec<String>,
    missing_locator_count: usize,
    supports: usize,
    contradicts: usize,
    depends: usize,
    flags: PacketFlags,
}

#[derive(Debug, Clone, Serialize)]
struct PacketCandidateGap {
    finding_id: String,
    assertion: String,
    confidence: f64,
    conditions: String,
    entities: Vec<String>,
    review_status: String,
}

#[derive(Debug, Clone, Serialize)]
struct PacketMcpSession {
    protocol: String,
    recommended_loop: Vec<String>,
    tool_catalog: serde_json::Value,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PacketCheckSummary {
    status: String,
    generated_at: String,
    checked_artifacts: Vec<String>,
    counts: PacketManifestStats,
    proposal_summary: crate::proposals::ProposalSummary,
    proof_state: crate::proposals::ProofState,
    caveats: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PacketProofTrace {
    trace_version: String,
    generated_at: String,
    source: String,
    source_hash: String,
    snapshot_hash: String,
    event_log_hash: String,
    proposal_state_hash: String,
    replay_status: String,
    packet_manifest_hash: Option<String>,
    schema_version: String,
    checked_artifacts: Vec<String>,
    caveats: Vec<String>,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
struct PacketLock {
    lock_format: String,
    generated_at: String,
    files: Vec<PacketManifestFile>,
}

#[derive(Debug, Clone, Serialize)]
struct PacketManifest {
    packet_format: String,
    packet_version: String,
    generated_at: String,
    source: PacketSource,
    stats: PacketManifestStats,
    included_files: Vec<PacketManifestFile>,
}

#[derive(Debug, Clone, Serialize)]
struct PacketSource {
    project_name: String,
    description: String,
    compiled_at: String,
    compiler: String,
    vela_version: String,
    schema: String,
}

#[derive(Debug, Clone, Serialize)]
struct PacketManifestStats {
    findings: usize,
    sources: usize,
    evidence_atoms: usize,
    condition_records: usize,
    review_events: usize,
    proposals: usize,
    gaps: usize,
    contested: usize,
    bridge_entities: usize,
    contradiction_edges: usize,
}

#[derive(Debug, Clone, Serialize)]
struct PacketManifestFile {
    path: String,
    sha256: String,
    bytes: usize,
}

#[derive(Debug, Clone)]
pub struct PacketExportRecord {
    pub generated_at: String,
    pub snapshot_hash: String,
    pub event_log_hash: String,
    pub packet_manifest_hash: String,
}

#[derive(Debug, Clone)]
struct PacketFile {
    path: String,
    content: Vec<u8>,
}

impl PacketFile {
    fn text(path: impl Into<String>, content: String) -> Self {
        Self {
            path: path.into(),
            content: content.into_bytes(),
        }
    }

    fn json<T: Serialize>(path: impl Into<String>, value: &T) -> Result<Self, String> {
        let content = serde_json::to_vec_pretty(value)
            .map_err(|e| format!("Failed to serialize packet file: {e}"))?;
        Ok(Self {
            path: path.into(),
            content,
        })
    }
}

pub fn export_packet(frontier: &Project, output_dir: &Path) -> Result<PacketExportRecord, String> {
    use std::fs;

    fs::create_dir_all(output_dir.join("findings"))
        .map_err(|e| format!("Failed to create findings dir: {e}"))?;
    fs::create_dir_all(output_dir.join("reviews"))
        .map_err(|e| format!("Failed to create reviews dir: {e}"))?;
    fs::create_dir_all(output_dir.join("sources"))
        .map_err(|e| format!("Failed to create sources dir: {e}"))?;
    fs::create_dir_all(output_dir.join("evidence"))
        .map_err(|e| format!("Failed to create evidence dir: {e}"))?;
    fs::create_dir_all(output_dir.join("conditions"))
        .map_err(|e| format!("Failed to create conditions dir: {e}"))?;
    fs::create_dir_all(output_dir.join("proposals"))
        .map_err(|e| format!("Failed to create proposals dir: {e}"))?;

    let generated_at = Utc::now().to_rfc3339();
    let source_evidence = sources::derive_projection(frontier);
    let source_records = source_evidence.sources;
    let evidence_atoms = source_evidence.evidence_atoms;
    let condition_records = source_evidence.condition_records;
    let mut atoms_by_finding: BTreeMap<String, Vec<&sources::EvidenceAtom>> = BTreeMap::new();
    for atom in &evidence_atoms {
        atoms_by_finding
            .entry(atom.finding_id.clone())
            .or_default()
            .push(atom);
    }

    let mut entity_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut entity_types: BTreeMap<String, String> = BTreeMap::new();
    let mut entity_categories: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut entity_finding_ids: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for finding in &frontier.findings {
        for entity in &finding.assertion.entities {
            *entity_counts.entry(entity.name.clone()).or_default() += 1;
            entity_types
                .entry(entity.name.clone())
                .or_insert_with(|| entity.entity_type.clone());
            *entity_categories
                .entry(entity.name.clone())
                .or_default()
                .entry(finding.assertion.assertion_type.clone())
                .or_default() += 1;
            entity_finding_ids
                .entry(entity.name.clone())
                .or_default()
                .insert(finding.id.clone());
        }
    }

    let mut top_entities: Vec<PacketEntitySummary> = entity_counts
        .iter()
        .map(|(name, finding_count)| PacketEntitySummary {
            name: name.clone(),
            entity_type: entity_types
                .get(name)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            finding_count: *finding_count,
            categories: entity_categories
                .get(name)
                .map(|cats| cats.keys().cloned().collect())
                .unwrap_or_default(),
        })
        .collect();
    top_entities.sort_by(|a, b| {
        b.finding_count
            .cmp(&a.finding_count)
            .then_with(|| a.name.cmp(&b.name))
    });
    top_entities.truncate(25);

    let overview = PacketOverview {
        project_name: frontier.project.name.clone(),
        description: frontier.project.description.clone(),
        compiled_at: frontier.project.compiled_at.clone(),
        generated_at: generated_at.clone(),
        findings: frontier.stats.findings,
        papers_processed: frontier.project.papers_processed,
        avg_confidence: frontier.stats.avg_confidence,
        categories: frontier
            .stats
            .categories
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect(),
        link_types: frontier
            .stats
            .link_types
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect(),
        top_entities,
    };

    let mut packet_findings: Vec<PacketFindingSummary> = frontier
        .findings
        .iter()
        .map(|finding| PacketFindingSummary {
            id: finding.id.clone(),
            assertion_type: finding.assertion.assertion_type.clone(),
            assertion_text: finding.assertion.text.clone(),
            confidence: finding.confidence.score,
            evidence_type: finding.evidence.evidence_type.clone(),
            method: finding.evidence.method.clone(),
            entities: finding
                .assertion
                .entities
                .iter()
                .map(|entity| entity.name.clone())
                .collect(),
            doi: finding.provenance.doi.clone(),
            source_title: finding.provenance.title.clone(),
            flags: PacketFlags {
                gap: finding.flags.gap,
                contested: finding.flags.contested,
                replicated: finding.evidence.replicated,
            },
            link_count: finding.links.len(),
        })
        .collect();
    packet_findings.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.link_count.cmp(&a.link_count))
            .then_with(|| a.id.cmp(&b.id))
    });

    let high_signal_findings: Vec<PacketFindingSummary> = packet_findings
        .iter()
        .filter(|finding| {
            finding.flags.gap
                || finding.flags.contested
                || finding.flags.replicated
                || finding.confidence >= 0.85
                || finding.link_count > 0
        })
        .take(50)
        .cloned()
        .collect();

    let gap_findings: Vec<PacketFindingSummary> = packet_findings
        .iter()
        .filter(|finding| finding.flags.gap)
        .cloned()
        .collect();

    let contested_findings: Vec<PacketFindingSummary> = packet_findings
        .iter()
        .filter(|finding| finding.flags.contested)
        .cloned()
        .collect();

    let mut bridge_entities: Vec<PacketBridgeSummary> = entity_categories
        .iter()
        .filter(|(_, categories)| categories.len() >= 2)
        .map(|(entity, categories)| PacketBridgeSummary {
            entity: entity.clone(),
            entity_type: entity_types
                .get(entity)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            categories: categories.clone(),
            finding_ids: entity_finding_ids
                .get(entity)
                .map(|ids| ids.iter().cloned().collect())
                .unwrap_or_default(),
        })
        .collect();
    bridge_entities.sort_by(|a, b| {
        b.categories
            .len()
            .cmp(&a.categories.len())
            .then_with(|| b.finding_ids.len().cmp(&a.finding_ids.len()))
            .then_with(|| a.entity.cmp(&b.entity))
    });

    let finding_lookup: HashMap<&str, &FindingBundle> = frontier
        .findings
        .iter()
        .map(|finding| (finding.id.as_str(), finding))
        .collect();
    let mut contradictions = Vec::new();
    let mut seen_pairs = BTreeSet::new();
    for finding in &frontier.findings {
        for link in &finding.links {
            if !(link.link_type == "contradicts" || link.link_type == "disputes") {
                continue;
            }
            let pair_key = if finding.id <= link.target {
                format!("{}::{}::{}", finding.id, link.target, link.link_type)
            } else {
                format!("{}::{}::{}", link.target, finding.id, link.link_type)
            };
            if !seen_pairs.insert(pair_key) {
                continue;
            }
            if let Some(target) = finding_lookup.get(link.target.as_str()) {
                contradictions.push(PacketContradictionSummary {
                    source_id: finding.id.clone(),
                    target_id: target.id.clone(),
                    link_type: link.link_type.clone(),
                    source_assertion: finding.assertion.text.clone(),
                    target_assertion: target.assertion.text.clone(),
                });
            }
        }
    }

    let caveats = packet_caveats();
    let scope = PacketScope {
        frontier_name: frontier.project.name.clone(),
        description: frontier.project.description.clone(),
        generated_at: generated_at.clone(),
        source_schema: frontier.schema.clone(),
        finding_count: frontier.findings.len(),
        papers_processed: frontier.project.papers_processed,
        review_event_count: frontier.review_events.len(),
        intended_use: vec![
            "Review a bounded compiled frontier".to_string(),
            "Inspect findings, evidence, confidence, provenance, and links".to_string(),
            "Compare candidate tensions, gaps, and bridges".to_string(),
            "Serve reviewable context to MCP/HTTP clients".to_string(),
        ],
        out_of_scope: vec![
            "Autonomous experiment planning".to_string(),
            "Definitive novelty claims".to_string(),
            "Institutional federation or broad exchange-network claims".to_string(),
        ],
        caveats: caveats.clone(),
    };

    let source_table: Vec<PacketSourceRow> = source_records
        .iter()
        .map(|source| PacketSourceRow {
            source_key: source.id.clone(),
            source_id: source.id.clone(),
            locator: source.locator.clone(),
            content_hash: source.content_hash.clone(),
            title: source.title.clone(),
            doi: source.doi.clone(),
            pmid: source.pmid.clone(),
            year: source.year,
            source_type: source.source_type.clone(),
            extraction_mode: source.extraction_mode.clone(),
            source_quality: source.source_quality.clone(),
            caveats: source.caveats.clone(),
            finding_ids: source.finding_ids.clone(),
        })
        .collect();

    let evidence_matrix: Vec<PacketEvidenceMatrixRow> = frontier
        .findings
        .iter()
        .map(|finding| {
            let atoms = atoms_by_finding
                .get(&finding.id)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let evidence_atom_ids = atoms.iter().map(|atom| atom.id.clone()).collect::<Vec<_>>();
            let source_id = atoms.first().map(|atom| atom.source_id.clone());
            let missing_locator_count = atoms.iter().filter(|atom| atom.locator.is_none()).count();
            let supports = finding
                .links
                .iter()
                .filter(|link| {
                    matches!(
                        link.link_type.as_str(),
                        "supports" | "extends" | "replicates"
                    )
                })
                .count();
            let contradicts = finding
                .links
                .iter()
                .filter(|link| matches!(link.link_type.as_str(), "contradicts" | "disputes"))
                .count();
            let depends = finding
                .links
                .iter()
                .filter(|link| link.link_type == "depends")
                .count();
            PacketEvidenceMatrixRow {
                finding_id: finding.id.clone(),
                assertion_type: finding.assertion.assertion_type.clone(),
                evidence_type: finding.evidence.evidence_type.clone(),
                method: finding.evidence.method.clone(),
                confidence: finding.confidence.score,
                replicated: finding.evidence.replicated,
                human_data: finding.conditions.human_data,
                clinical_trial: finding.conditions.clinical_trial,
                source_key: source_key(finding),
                source_id,
                evidence_atom_ids,
                missing_locator_count,
                supports,
                contradicts,
                depends,
                flags: PacketFlags {
                    gap: finding.flags.gap,
                    contested: finding.flags.contested,
                    replicated: finding.evidence.replicated,
                },
            }
        })
        .collect();

    let candidate_gaps: Vec<PacketCandidateGap> = frontier
        .findings
        .iter()
        .filter(|finding| finding.flags.gap)
        .map(|finding| PacketCandidateGap {
            finding_id: finding.id.clone(),
            assertion: finding.assertion.text.clone(),
            confidence: finding.confidence.score,
            conditions: finding.conditions.text.clone(),
            entities: finding
                .assertion
                .entities
                .iter()
                .map(|entity| entity.name.clone())
                .collect(),
            review_status: finding
                .provenance
                .review
                .as_ref()
                .map(|review| {
                    if review.reviewed {
                        "reviewed".to_string()
                    } else {
                        "unreviewed".to_string()
                    }
                })
                .unwrap_or_else(|| "unreviewed".to_string()),
        })
        .collect();

    let mcp_session = PacketMcpSession {
        protocol: "model-context-protocol".to_string(),
        recommended_loop: vec![
            "frontier_stats".to_string(),
            "search_findings".to_string(),
            "get_finding".to_string(),
            "list_gaps".to_string(),
            "find_bridges".to_string(),
            "check_pubmed".to_string(),
            "list_contradictions".to_string(),
            "propagate_retraction".to_string(),
            "apply_observer".to_string(),
        ],
        tool_catalog: crate::tool_registry::mcp_tools_json(),
        notes: caveats.clone(),
    };

    let stats = PacketManifestStats {
        findings: frontier.findings.len(),
        sources: source_records.len(),
        evidence_atoms: evidence_atoms.len(),
        condition_records: condition_records.len(),
        review_events: frontier.review_events.len(),
        proposals: frontier.proposals.len(),
        gaps: gap_findings.len(),
        contested: contested_findings.len(),
        bridge_entities: bridge_entities.len(),
        contradiction_edges: contradictions.len(),
    };

    // Phase K: `checked_artifacts` carries the proof-bearing surface —
    // canonical artifacts only. Derived projections (signals, queues,
    // tables, candidate-*) ship in the packet but are regenerable from
    // canonical inputs and are not proof-load-bearing.
    let checked_artifacts = packet::canonical_packet_files()
        .iter()
        .map(|path| (*path).to_string())
        .collect::<Vec<_>>();

    let check_summary = PacketCheckSummary {
        status: "ok".to_string(),
        generated_at: generated_at.clone(),
        checked_artifacts: checked_artifacts.clone(),
        counts: stats.clone(),
        proposal_summary: crate::proposals::summary(frontier),
        proof_state: frontier.proof_state.clone(),
        caveats: caveats.clone(),
    };
    let signal_report = signals::analyze(frontier, &[]);
    let quality_table = signals::quality_table(frontier, &signal_report);
    let state_transitions = state::state_transitions(frontier);
    let replay_report = events::replay_report(frontier);
    let ro_crate = signals::ro_crate_metadata(frontier, &checked_artifacts);

    let frontier_bytes = crate::canonical::to_canonical_bytes(frontier)
        .map_err(|e| format!("Failed to serialize frontier for source hash: {e}"))?;
    let proof_trace = PacketProofTrace {
        trace_version: "0.2.0".to_string(),
        generated_at: generated_at.clone(),
        source: frontier.project.name.clone(),
        source_hash: hex::encode(Sha256::digest(&frontier_bytes)),
        snapshot_hash: replay_report.current_hash.clone(),
        event_log_hash: replay_report.event_log_hash.clone(),
        proposal_state_hash: crate::proposals::proposal_state_hash(&frontier.proposals),
        replay_status: replay_report.status.clone(),
        packet_manifest_hash: None,
        schema_version: frontier.vela_version.clone(),
        checked_artifacts: checked_artifacts.clone(),
        caveats: caveats.clone(),
        status: "ok".to_string(),
    };

    let readme = export_packet_readme(
        frontier,
        &generated_at,
        high_signal_findings.len(),
        gap_findings.len(),
        contested_findings.len(),
        bridge_entities.len(),
        contradictions.len(),
    );

    let mut files = vec![
        PacketFile::text("README.md", readme),
        PacketFile::text("reviewer-guide.md", export_reviewer_guide(frontier)),
        PacketFile::json("overview.json", &overview)?,
        PacketFile::json("scope.json", &scope)?,
        PacketFile::json("source-table.json", &source_table)?,
        PacketFile::json("sources/source-registry.json", &source_records)?,
        PacketFile::json("evidence-matrix.json", &evidence_matrix)?,
        PacketFile::json("evidence/evidence-atoms.json", &evidence_atoms)?,
        PacketFile::json(
            "evidence/source-evidence-map.json",
            &sources::source_evidence_map_from_atoms(&evidence_atoms),
        )?,
        PacketFile::json("conditions/condition-records.json", &condition_records)?,
        PacketFile::json(
            "conditions/condition-matrix.json",
            &sources::condition_matrix(&condition_records),
        )?,
        PacketFile::json("signals.json", &signal_report.signals)?,
        PacketFile::json("review-queue.json", &signal_report.review_queue)?,
        PacketFile::json("quality-table.json", &quality_table)?,
        PacketFile::json("state-transitions.json", &state_transitions)?,
        PacketFile::json("events/events.json", &frontier.events)?,
        PacketFile::json("events/replay-report.json", &replay_report)?,
        PacketFile::json("proposals/proposals.json", &frontier.proposals)?,
        PacketFile::json("ro-crate-metadata.jsonld", &ro_crate)?,
        PacketFile::json("candidate-tensions.json", &contradictions)?,
        PacketFile::json("candidate-gaps.json", &candidate_gaps)?,
        PacketFile::json("candidate-bridges.json", &bridge_entities)?,
        PacketFile::json("mcp-session.json", &mcp_session)?,
        PacketFile::json("check-summary.json", &check_summary)?,
        PacketFile::json("proof-trace.json", &proof_trace)?,
        PacketFile::json("findings/high-signal.json", &high_signal_findings)?,
        PacketFile::json("findings/full.json", &frontier.findings)?,
        PacketFile::json("findings/gaps.json", &gap_findings)?,
        PacketFile::json("findings/contested.json", &contested_findings)?,
        PacketFile::json("findings/bridges.json", &bridge_entities)?,
        PacketFile::json("findings/contradictions.json", &contradictions)?,
        PacketFile::json("reviews/review-events.json", &frontier.review_events)?,
        PacketFile::json(
            "reviews/confidence-updates.json",
            &frontier.confidence_updates,
        )?,
    ];

    let lock = PacketLock {
        lock_format: "vela.packet-lock.v1".to_string(),
        generated_at: generated_at.clone(),
        files: files.iter().map(manifest_entry_for_file).collect(),
    };
    files.push(PacketFile::json("packet.lock.json", &lock)?);

    for file in &files {
        let full_path = output_dir.join(&file.path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Failed to create packet parent dir {}: {e}",
                    parent.display()
                )
            })?;
        }
        fs::write(&full_path, &file.content)
            .map_err(|e| format!("Failed to write packet file {}: {e}", file.path))?;
    }

    let manifest = PacketManifest {
        packet_format: "vela.frontier-packet".to_string(),
        packet_version: "v1".to_string(),
        generated_at: generated_at.clone(),
        source: PacketSource {
            project_name: frontier.project.name.clone(),
            description: frontier.project.description.clone(),
            compiled_at: frontier.project.compiled_at.clone(),
            compiler: frontier.project.compiler.clone(),
            vela_version: frontier.vela_version.clone(),
            schema: frontier.schema.clone(),
        },
        stats,
        included_files: files
            .drain(..)
            .map(|file| manifest_entry_for_file(&file))
            .collect(),
    };

    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize packet manifest: {e}"))?;
    let manifest_path = output_dir.join("manifest.json");
    fs::write(&manifest_path, &manifest_bytes)
        .map_err(|e| format!("Failed to write manifest.json: {e}"))?;

    let packet_manifest_hash = hex::encode(Sha256::digest(&manifest_bytes));
    Ok(PacketExportRecord {
        generated_at,
        snapshot_hash: replay_report.current_hash,
        event_log_hash: replay_report.event_log_hash,
        packet_manifest_hash,
    })
}

fn export_packet_readme(
    frontier: &Project,
    generated_at: &str,
    high_signal_count: usize,
    gap_count: usize,
    contested_count: usize,
    bridge_count: usize,
    contradiction_count: usize,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {} packet\n\n", frontier.project.name));
    out.push_str(&format!("{}\n\n", frontier.project.description));
    out.push_str("This export is a bounded network packet: a compact, publishable subset of the frontier optimized for review, contradiction inspection, and grounded agent context. It intentionally does not dump the full raw frontier by default.\n\n");
    out.push_str("## Source\n\n");
    out.push_str(&format!("- Project: {}\n", frontier.project.name));
    out.push_str(&format!(
        "- Compiled at: {}\n",
        frontier.project.compiled_at
    ));
    out.push_str(&format!("- Generated at: {}\n", generated_at));
    out.push_str(&format!("- Compiler: {}\n", frontier.project.compiler));
    out.push_str(&format!("- Vela version: {}\n", frontier.vela_version));
    out.push_str(&format!("- Schema: {}\n\n", frontier.schema));
    out.push_str("## Included artifacts\n\n");
    out.push_str("- `manifest.json` — provenance, version stamp, checksums\n");
    out.push_str("- `overview.json` — project-level stats, categories, top entities\n");
    out.push_str("- `findings/high-signal.json` — compact high-signal finding subset\n");
    out.push_str(
        "- `findings/full.json` — canonical finding bundles for packet import and merge\n",
    );
    out.push_str("- `findings/gaps.json` — gap-tagged findings\n");
    out.push_str("- `findings/contested.json` — contested findings\n");
    out.push_str("- `findings/bridges.json` — entities spanning multiple assertion categories\n");
    out.push_str("- `findings/contradictions.json` — explicit contradiction/dispute edges\n");
    out.push_str("- `reviews/review-events.json` — attached review events\n");
    out.push_str("- `reviews/confidence-updates.json` — interpretation confidence revisions\n");
    out.push_str("- `state-transitions.json` — combined review and confidence transition log\n\n");
    out.push_str("## Packet stats\n\n");
    out.push_str(&format!(
        "- Findings in source frontier: {}\n",
        frontier.findings.len()
    ));
    out.push_str(&format!(
        "- High-signal findings exported: {}\n",
        high_signal_count
    ));
    out.push_str(&format!("- Gap findings exported: {}\n", gap_count));
    out.push_str(&format!(
        "- Contested findings exported: {}\n",
        contested_count
    ));
    out.push_str(&format!("- Bridge entities exported: {}\n", bridge_count));
    out.push_str(&format!(
        "- Contradiction edges exported: {}\n",
        contradiction_count
    ));
    out.push_str(&format!(
        "- Review events exported: {}\n",
        frontier.review_events.len()
    ));
    out
}

fn export_reviewer_guide(frontier: &Project) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Reviewer guide: {}\n\n", frontier.project.name));
    out.push_str("Use this packet as a reviewable frontier snapshot. Start with `scope.json`, then inspect `evidence-matrix.json`, `candidate-tensions.json`, `candidate-gaps.json`, and `candidate-bridges.json` before reading individual finding bundles.\n\n");
    out.push_str("## Suggested review loop\n\n");
    out.push_str(
        "1. Confirm the bounded scope and source corpus in `scope.json`, `source-table.json`, and `sources/source-registry.json`.\n",
    );
    out.push_str("2. Check high-confidence or high-link findings in `evidence-matrix.json`, then inspect exact source-grounded atoms in `evidence/evidence-atoms.json`.\n");
    out.push_str(
        "3. Inspect candidate tensions against the full finding bundles in `findings/full.json`.\n",
    );
    out.push_str(
        "4. Treat candidate gaps and bridges as leads requiring review, not as settled claims.\n",
    );
    out.push_str("5. Use `mcp-session.json` to replay the conservative MCP investigation loop.\n");
    out.push_str("6. Verify checksums with `manifest.json` and `packet.lock.json` before comparing packet diffs.\n\n");
    out.push_str("## Caveats\n\n");
    for caveat in packet_caveats() {
        out.push_str(&format!("- {caveat}\n"));
    }
    out
}

fn packet_caveats() -> Vec<String> {
    vec![
        "Candidate contradictions, gaps, and bridges require human review.".to_string(),
        "Evidence ranking is heuristic: meta-analysis > RCT > cohort > case-control > case-report > in-vitro.".to_string(),
        "PubMed prior-art checks are rough signals, not proof of novelty.".to_string(),
        "Observer policy output is weighted reranking, not definitive disagreement.".to_string(),
        "Retraction impact is simulated over declared dependency links.".to_string(),
    ]
}

fn source_key(finding: &FindingBundle) -> String {
    if let Some(doi) = &finding.provenance.doi {
        return format!("doi:{doi}");
    }
    if let Some(pmid) = &finding.provenance.pmid {
        return format!("pmid:{pmid}");
    }
    format!("title:{}", finding.provenance.title)
}

fn manifest_entry_for_file(file: &PacketFile) -> PacketManifestFile {
    PacketManifestFile {
        path: file.path.clone(),
        sha256: hex::encode(Sha256::digest(&file.content)),
        bytes: file.content.len(),
    }
}

// ── Markdown ─────────────────────────────────────────────────────────────────

fn export_markdown(frontier: &Project) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", frontier.project.name));
    out.push_str(&format!("{}\n\n", frontier.project.description));
    out.push_str(&format!(
        "**Findings:** {} | **Papers:** {} | **Avg confidence:** {:.2}\n\n",
        frontier.stats.findings, frontier.project.papers_processed, frontier.stats.avg_confidence
    ));

    // Group by assertion type.
    let mut by_type: BTreeMap<String, Vec<&FindingBundle>> = BTreeMap::new();
    for f in &frontier.findings {
        by_type
            .entry(f.assertion.assertion_type.clone())
            .or_default()
            .push(f);
    }

    for (atype, findings) in &by_type {
        out.push_str(&format!("## {} ({})\n\n", atype, findings.len()));

        for f in findings {
            let entities: Vec<&str> = f
                .assertion
                .entities
                .iter()
                .map(|e| e.name.as_str())
                .collect();
            let repl = if f.evidence.replicated {
                " [replicated]"
            } else {
                ""
            };
            let gap = if f.flags.gap { " [GAP]" } else { "" };
            let contested = if f.flags.contested {
                " [CONTESTED]"
            } else {
                ""
            };

            out.push_str(&format!(
                "- **[{:.2}]** {}{}{}{}\n",
                f.confidence.score, f.assertion.text, repl, gap, contested
            ));
            if !entities.is_empty() {
                out.push_str(&format!("  - Entities: {}\n", entities.join(", ")));
            }
            if let Some(doi) = &f.provenance.doi {
                let year = f.provenance.year.map(|y| y.to_string()).unwrap_or_default();
                out.push_str(&format!(
                    "  - Source: {} ({}) [doi:{}](https://doi.org/{})\n",
                    f.provenance.title, year, doi, doi
                ));
            }
            out.push('\n');
        }
    }

    out
}

// ── Nanopub validation ─────────────────────────────────────────────────────

/// Validate a JSON-LD export against nanopub structural expectations.
///
/// Returns a list of validation warnings. An empty list means the export
/// passes all checks. This is not a full nanopub spec validator, but it
/// catches the most common structural issues for interoperability.
pub fn validate_nanopub(jsonld: &str) -> Vec<String> {
    let mut warnings = Vec::new();

    let doc: serde_json::Value = match serde_json::from_str(jsonld) {
        Ok(v) => v,
        Err(e) => {
            warnings.push(format!("Invalid JSON: {e}"));
            return warnings;
        }
    };

    // Check top-level @context exists
    if doc.get("@context").is_none() {
        warnings.push("Missing top-level @context".into());
    }

    let graph = match doc["@graph"].as_array() {
        Some(g) => g,
        None => {
            warnings.push("Missing or invalid @graph array".into());
            return warnings;
        }
    };

    for (i, node) in graph.iter().enumerate() {
        let label = node["@id"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("graph[{}]", i));

        // Every finding must have @type
        if node.get("@type").is_none() {
            warnings.push(format!("{}: missing @type", label));
        }

        // Provenance must include source information
        let activity = &node["prov:wasGeneratedBy"];
        if activity.is_null() {
            warnings.push(format!(
                "{}: missing prov:wasGeneratedBy (no provenance activity)",
                label
            ));
        } else if activity["prov:used"].is_null() {
            warnings.push(format!(
                "{}: provenance activity has no prov:used (no source DOI)",
                label
            ));
        }

        // Assertions should have entities with identifiers
        if let Some(entities) = node["vela:hasEntity"].as_array() {
            for (j, entity) in entities.iter().enumerate() {
                let ename = entity["vela:entityName"].as_str().unwrap_or("unknown");
                if entity.get("schema:identifier").is_none() {
                    warnings.push(format!(
                        "{}: entity {} ('{}') has no schema:identifier",
                        label, j, ename
                    ));
                }
            }
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::*;
    use crate::project;

    fn make_frontier() -> Project {
        let f1 = FindingBundle {
            id: "vf_abc123".into(),
            version: 1,
            previous_version: None,
            assertion: Assertion {
                text: "NLRP3 activates caspase-1".into(),
                assertion_type: "mechanism".into(),
                entities: vec![
                    Entity {
                        name: "NLRP3".into(),
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
                    },
                    Entity {
                        name: "caspase-1".into(),
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
                    },
                ],
                relation: Some("activates".into()),
                direction: Some("positive".into()),
            },
            evidence: Evidence {
                evidence_type: "experimental".into(),
                model_system: "mouse".into(),
                species: Some("Mus musculus".into()),
                method: "Western blot".into(),
                sample_size: None,
                effect_size: None,
                p_value: None,
                replicated: true,
                replication_count: None,
                evidence_spans: vec![],
            },
            conditions: Conditions {
                text: "In vitro".into(),
                species_verified: vec![],
                species_unverified: vec![],
                in_vitro: true,
                in_vivo: false,
                human_data: false,
                clinical_trial: false,
                concentration_range: None,
                duration: None,
                age_group: None,
                cell_type: None,
            },
            confidence: Confidence::legacy(0.9, "grounded", 0.85),
            provenance: Provenance {
                source_type: "published_paper".into(),
                doi: Some("10.1234/test".into()),
                pmid: None,
                pmc: None,
                openalex_id: None,
                url: None,
                title: "NLRP3 inflammasome paper".into(),
                authors: vec![Author {
                    name: "Smith J".into(),
                    orcid: None,
                }],
                year: Some(2023),
                journal: Some("Nature".into()),
                license: None,
                publisher: None,
                funders: vec![],
                extraction: Extraction::default(),
                review: None,
                citation_count: Some(50),
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
        };

        project::assemble("Test frontier", vec![f1], 1, 0, "Test description")
    }

    #[test]
    fn csv_has_header_and_row() {
        let c = make_frontier();
        let csv = export_csv(&c);
        let lines: Vec<&str> = csv.lines().collect();
        assert!(lines[0].starts_with("id,"));
        assert_eq!(lines.len(), 2); // header + 1 finding
        assert!(lines[1].contains("NLRP3"));
    }

    #[test]
    fn jsonld_valid_json() {
        let c = make_frontier();
        let jsonld = export_jsonld(&c);
        let parsed: serde_json::Value = serde_json::from_str(&jsonld).unwrap();
        // Verify context has nanopub-inspired vocabulary
        let ctx = &parsed["@context"];
        assert_eq!(ctx["@vocab"], "https://vela.science/schema/");
        assert_eq!(ctx["schema"], "https://schema.org/");
        assert_eq!(ctx["prov"], "http://www.w3.org/ns/prov#");
        assert_eq!(ctx["np"], "http://www.nanopub.org/nschema#");
        let graph = parsed["@graph"].as_array().unwrap();
        assert_eq!(graph.len(), 1);
        assert_eq!(graph[0]["@type"], "vela:FindingBundle");
    }

    #[test]
    fn jsonld_finding_fields() {
        let c = make_frontier();
        let jsonld = export_jsonld(&c);
        let parsed: serde_json::Value = serde_json::from_str(&jsonld).unwrap();
        let node = &parsed["@graph"][0];
        assert_eq!(node["@id"], "vela:vf_abc123");
        assert_eq!(node["vela:assertionType"], "mechanism");
        assert_eq!(node["vela:confidence"], 0.9);
        assert_eq!(node["vela:evidenceType"], "experimental");
        // Provenance should reference DOI
        let activity = &node["prov:wasGeneratedBy"];
        assert_eq!(activity["prov:used"]["@id"], "doi:10.1234/test");
    }

    #[test]
    fn jsonld_entities_present() {
        let c = make_frontier();
        let jsonld = export_jsonld(&c);
        let parsed: serde_json::Value = serde_json::from_str(&jsonld).unwrap();
        let entities = parsed["@graph"][0]["vela:hasEntity"].as_array().unwrap();
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0]["vela:entityName"], "NLRP3");
        assert_eq!(entities[0]["vela:entityType"], "protein");
    }

    #[test]
    fn jsonld_roundtrip_valid() {
        let c = make_frontier();
        let jsonld = export_jsonld(&c);
        // Verify it parses back to valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&jsonld).unwrap();
        // Re-serialize and parse again to confirm stability
        let re_serialized = serde_json::to_string_pretty(&parsed).unwrap();
        let re_parsed: serde_json::Value = serde_json::from_str(&re_serialized).unwrap();
        assert_eq!(parsed, re_parsed);
    }

    #[test]
    fn bibtex_has_entry() {
        let c = make_frontier();
        let bib = export_bibtex(&c);
        assert!(bib.contains("@article{"));
        assert!(bib.contains("NLRP3 inflammasome paper"));
    }

    #[test]
    fn markdown_has_heading() {
        let c = make_frontier();
        let md = export_markdown(&c);
        assert!(md.starts_with("# Test frontier"));
        assert!(md.contains("## mechanism"));
    }

    #[test]
    fn csv_escape_handles_commas() {
        assert_eq!(csv_escape("hello,world"), "\"hello,world\"");
        assert_eq!(csv_escape("plain"), "plain");
    }

    #[test]
    fn format_parsing() {
        assert!(ExportFormat::from_str("csv").is_ok());
        assert!(ExportFormat::from_str("jsonld").is_ok());
        assert!(ExportFormat::from_str("json-ld").is_ok());
        assert!(ExportFormat::from_str("bibtex").is_ok());
        assert!(ExportFormat::from_str("bib").is_ok());
        assert!(ExportFormat::from_str("markdown").is_ok());
        assert!(ExportFormat::from_str("md").is_ok());
        assert!(ExportFormat::from_str("packet").is_ok());
        assert!(ExportFormat::from_str("wiki").is_err());
        assert!(ExportFormat::from_str("obsidian").is_err());
        assert!(ExportFormat::from_str("xml").is_err());
    }

    #[test]
    fn multi_file_formats_are_flagged() {
        let packet = ExportFormat::from_str("packet").unwrap();
        assert!(packet.is_multi_file());
        let csv = ExportFormat::from_str("csv").unwrap();
        assert!(!csv.is_multi_file());
    }

    #[test]
    fn packet_export_creates_manifest_and_payload_files() {
        let c = make_frontier();
        let dir = std::env::temp_dir().join(format!("vela_packet_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        export_packet(&c, &dir).unwrap();

        assert!(dir.join("README.md").exists());
        assert!(dir.join("reviewer-guide.md").exists());
        assert!(dir.join("manifest.json").exists());
        assert!(dir.join("overview.json").exists());
        assert!(dir.join("scope.json").exists());
        assert!(dir.join("source-table.json").exists());
        assert!(dir.join("sources/source-registry.json").exists());
        assert!(dir.join("evidence-matrix.json").exists());
        assert!(dir.join("evidence/evidence-atoms.json").exists());
        assert!(dir.join("evidence/source-evidence-map.json").exists());
        assert!(dir.join("conditions/condition-records.json").exists());
        assert!(dir.join("conditions/condition-matrix.json").exists());
        assert!(dir.join("candidate-tensions.json").exists());
        assert!(dir.join("candidate-gaps.json").exists());
        assert!(dir.join("candidate-bridges.json").exists());
        assert!(dir.join("mcp-session.json").exists());
        assert!(dir.join("check-summary.json").exists());
        assert!(dir.join("signals.json").exists());
        assert!(dir.join("review-queue.json").exists());
        assert!(dir.join("quality-table.json").exists());
        assert!(dir.join("state-transitions.json").exists());
        assert!(dir.join("events/events.json").exists());
        assert!(dir.join("events/replay-report.json").exists());
        assert!(dir.join("ro-crate-metadata.jsonld").exists());
        assert!(dir.join("proof-trace.json").exists());
        assert!(dir.join("packet.lock.json").exists());
        assert!(dir.join("findings/high-signal.json").exists());
        assert!(dir.join("findings/full.json").exists());
        assert!(dir.join("findings/gaps.json").exists());
        assert!(dir.join("findings/contested.json").exists());
        assert!(dir.join("findings/bridges.json").exists());
        assert!(dir.join("findings/contradictions.json").exists());
        assert!(dir.join("reviews/review-events.json").exists());
        assert!(dir.join("reviews/confidence-updates.json").exists());

        let readme = std::fs::read_to_string(dir.join("README.md")).unwrap();
        assert!(readme.contains("bounded network packet"));
        assert!(readme.contains("manifest.json"));

        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["packet_format"], "vela.frontier-packet");
        assert_eq!(manifest["packet_version"], "v1");
        assert_eq!(manifest["stats"]["findings"], 1);
        assert_eq!(manifest["stats"]["sources"], 1);
        assert_eq!(manifest["stats"]["evidence_atoms"], 1);
        assert_eq!(manifest["stats"]["condition_records"], 1);
        assert_eq!(manifest["included_files"].as_array().unwrap().len(), 34);

        let high_signal: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("findings/high-signal.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(high_signal.as_array().unwrap().len(), 1);
        assert_eq!(high_signal[0]["id"], "vf_abc123");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nanopub_validates_well_formed_jsonld() {
        let c = make_frontier();
        let jsonld = export_jsonld(&c);
        let warnings = validate_nanopub(&jsonld);
        // Only entity-identifier warnings expected (test entities are unresolved)
        for w in &warnings {
            assert!(w.contains("schema:identifier"), "Unexpected warning: {w}");
        }
    }

    #[test]
    fn nanopub_catches_invalid_json() {
        let warnings = validate_nanopub("not valid json {{{");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Invalid JSON"));
    }

    #[test]
    fn nanopub_catches_missing_graph() {
        let warnings = validate_nanopub(r#"{"@context": {}}"#);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("@graph"));
    }

    #[test]
    fn nanopub_catches_missing_type() {
        let doc = serde_json::json!({
            "@context": {},
            "@graph": [{"@id": "vela:test"}]
        });
        let warnings = validate_nanopub(&doc.to_string());
        assert!(warnings.iter().any(|w| w.contains("missing @type")));
    }
}
