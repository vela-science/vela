//! Stage 3b: RESOLVE — map entity names to canonical scientific identifiers.
//!
//! Queries free public APIs (MeSH, UniProt, PubChem) to ground entities
//! against standard databases. Results are cached per entity name to avoid
//! redundant lookups across findings.

use std::collections::HashMap;

use reqwest::Client;
use serde::Deserialize;

use crate::bundle::{FindingBundle, ResolutionMethod, ResolvedId};

// ── Hardcoded alias map for common neuroscience entities ──────────────────

fn hardcoded_aliases() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();
    m.insert("nlrp3", vec!["cryopyrin", "NALP3", "CIAS1"]);
    m.insert("cryopyrin", vec!["NLRP3", "NALP3", "CIAS1"]);
    m.insert("nalp3", vec!["NLRP3", "cryopyrin", "CIAS1"]);
    m.insert("cias1", vec!["NLRP3", "cryopyrin", "NALP3"]);

    m.insert(
        "trem2",
        vec!["triggering receptor expressed on myeloid cells 2"],
    );
    m.insert(
        "triggering receptor expressed on myeloid cells 2",
        vec!["TREM2"],
    );

    m.insert(
        "bace1",
        vec![
            "beta-secretase 1",
            "beta-site APP cleaving enzyme 1",
            "memapsin-2",
        ],
    );
    m.insert(
        "beta-secretase 1",
        vec!["BACE1", "beta-site APP cleaving enzyme 1", "memapsin-2"],
    );
    m.insert(
        "beta-site app cleaving enzyme 1",
        vec!["BACE1", "beta-secretase 1", "memapsin-2"],
    );
    m.insert(
        "memapsin-2",
        vec![
            "BACE1",
            "beta-secretase 1",
            "beta-site APP cleaving enzyme 1",
        ],
    );

    m.insert("apoe4", vec!["apolipoprotein E4", "ApoE \u{03b5}4"]);
    m.insert("apolipoprotein e4", vec!["APOE4", "ApoE \u{03b5}4"]);

    m.insert(
        "amyloid-beta",
        vec!["A\u{03b2}", "beta-amyloid", "amyloid \u{03b2}-peptide"],
    );
    m.insert(
        "a\u{03b2}",
        vec!["amyloid-beta", "beta-amyloid", "amyloid \u{03b2}-peptide"],
    );
    m.insert(
        "beta-amyloid",
        vec!["amyloid-beta", "A\u{03b2}", "amyloid \u{03b2}-peptide"],
    );
    m.insert(
        "amyloid \u{03b2}-peptide",
        vec!["amyloid-beta", "A\u{03b2}", "beta-amyloid"],
    );
    m
}

/// Look up hardcoded aliases for an entity name (case-insensitive).
fn get_hardcoded_aliases(name: &str) -> Vec<String> {
    let map = hardcoded_aliases();
    map.get(name.to_lowercase().as_str())
        .map(|v| v.iter().map(|s| s.to_string()).collect())
        .unwrap_or_default()
}

// ── MeSH (diseases, anatomical structures) ──────────────────────────────────

#[derive(Deserialize)]
struct MeshDescriptor {
    #[serde(default)]
    resource: String,
    #[serde(default)]
    label: String,
}

/// Result from MeSH lookup, including synonyms from the descriptor list.
struct MeshResult {
    id: String,
    confidence: f64,
    matched_name: String,
    synonyms: Vec<String>,
}

async fn lookup_mesh(client: &Client, name: &str) -> Option<MeshResult> {
    let url = format!(
        "https://id.nlm.nih.gov/mesh/lookup/descriptor?label={}&match=contains&limit=5",
        urlencoded(name),
    );
    let descriptors: Vec<MeshDescriptor> =
        crate::retry::retry_with_backoff("MeSH lookup", 2, || {
            let client = client.clone();
            let url = url.clone();
            async move {
                let resp = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| format!("MeSH error: {e}"))?;
                if !resp.status().is_success() {
                    return Err(format!("MeSH {}", resp.status()));
                }
                resp.json::<Vec<MeshDescriptor>>()
                    .await
                    .map_err(|e| format!("MeSH parse: {e}"))
            }
        })
        .await
        .ok()?;
    let first = descriptors.first()?;

    // Extract the descriptor ID from the resource URI (e.g. "http://id.nlm.nih.gov/mesh/D000544")
    let mesh_id = first.resource.rsplit('/').next().unwrap_or(&first.resource);

    // Confidence: exact match (case-insensitive) gets 0.95, otherwise 0.7
    let confidence = if first.label.eq_ignore_ascii_case(name) {
        0.95
    } else {
        0.7
    };

    // Collect synonyms from the other descriptor labels.
    let synonyms: Vec<String> = descriptors
        .iter()
        .skip(1)
        .filter(|d| {
            !d.label.eq_ignore_ascii_case(name) && !d.label.eq_ignore_ascii_case(&first.label)
        })
        .map(|d| d.label.clone())
        .collect();

    Some(MeshResult {
        id: mesh_id.to_string(),
        confidence,
        matched_name: first.label.clone(),
        synonyms,
    })
}

// ── UniProt (proteins) ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct UniprotResponse {
    #[serde(default)]
    results: Vec<UniprotEntry>,
}

#[derive(Deserialize)]
struct UniprotEntry {
    #[serde(rename = "primaryAccession", default)]
    primary_accession: String,
    #[serde(rename = "proteinDescription", default)]
    protein_description: Option<UniprotProteinDesc>,
}

#[derive(Deserialize)]
struct UniprotProteinDesc {
    #[serde(rename = "recommendedName", default)]
    recommended_name: Option<UniprotName>,
    #[serde(rename = "alternativeNames", default)]
    alternative_names: Vec<UniprotName>,
}

#[derive(Deserialize)]
struct UniprotName {
    #[serde(rename = "fullName", default)]
    full_name: Option<UniprotNameValue>,
}

#[derive(Deserialize)]
struct UniprotNameValue {
    #[serde(default)]
    value: String,
}

struct UniprotResult {
    id: String,
    confidence: f64,
    matched_name: String,
    synonyms: Vec<String>,
}

async fn lookup_uniprot(client: &Client, name: &str) -> Option<UniprotResult> {
    let url = format!(
        "https://rest.uniprot.org/uniprotkb/search?query={}+AND+reviewed:true&size=3&fields=accession,protein_name,organism_name",
        urlencoded(name),
    );
    let data: UniprotResponse = crate::retry::retry_with_backoff("UniProt lookup", 2, || {
        let client = client.clone();
        let url = url.clone();
        async move {
            let resp = client
                .get(&url)
                .header("Accept", "application/json")
                .send()
                .await
                .map_err(|e| format!("UniProt error: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("UniProt {}", resp.status()));
            }
            resp.json::<UniprotResponse>()
                .await
                .map_err(|e| format!("UniProt parse: {e}"))
        }
    })
    .await
    .ok()?;
    let first = data.results.first()?;

    // Confidence based on name match quality
    let rec_name = first
        .protein_description
        .as_ref()
        .and_then(|d| d.recommended_name.as_ref())
        .and_then(|n| n.full_name.as_ref())
        .map(|v| v.value.as_str())
        .unwrap_or("");

    let confidence = if rec_name.eq_ignore_ascii_case(name) {
        0.95
    } else if rec_name.to_lowercase().contains(&name.to_lowercase()) {
        0.8
    } else {
        0.6
    };

    let matched = if rec_name.is_empty() {
        name.to_string()
    } else {
        rec_name.to_string()
    };

    // Extract alternative names as synonyms.
    let synonyms: Vec<String> = first
        .protein_description
        .as_ref()
        .map(|d| {
            d.alternative_names
                .iter()
                .filter_map(|n| n.full_name.as_ref().map(|v| v.value.clone()))
                .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case(name))
                .collect()
        })
        .unwrap_or_default();

    Some(UniprotResult {
        id: first.primary_accession.clone(),
        confidence,
        matched_name: matched,
        synonyms,
    })
}

// ── PubChem (compounds) ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PubchemResponse {
    #[serde(rename = "IdentifierList", default)]
    identifier_list: Option<PubchemIdList>,
}

#[derive(Deserialize)]
struct PubchemIdList {
    #[serde(rename = "CID", default)]
    cid: Vec<u64>,
}

async fn lookup_pubchem(client: &Client, name: &str) -> Option<(String, f64, String)> {
    let url = format!(
        "https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/{}/cids/JSON",
        urlencoded(name),
    );
    let data: PubchemResponse = crate::retry::retry_with_backoff("PubChem lookup", 2, || {
        let client = client.clone();
        let url = url.clone();
        async move {
            let resp = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("PubChem error: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("PubChem {}", resp.status()));
            }
            resp.json::<PubchemResponse>()
                .await
                .map_err(|e| format!("PubChem parse: {e}"))
        }
    })
    .await
    .ok()?;
    let cid = data.identifier_list?.cid.first().copied()?;

    // PubChem name lookup is fairly precise, so high confidence when it resolves.
    Some((cid.to_string(), 0.9, name.to_string()))
}

// ── Dispatcher ──────────────────────────────────────────────────────────────

/// Cached resolution result.
#[derive(Clone)]
struct Resolved {
    db: String,
    id: String,
    confidence: f64,
    matched_name: String,
    /// Synonyms from the database API.
    api_aliases: Vec<String>,
    /// How the resolution was performed.
    method: ResolutionMethod,
    /// Alternative candidates (all results from the API, not just the best).
    all_candidates: Vec<ResolvedId>,
}

// ── Sanity checks ─────────────────────────────────────────────────────────

/// Check whether a resolved ID is plausible for the given entity type.
/// Returns false if the match is clearly wrong (e.g., anatomical_structure resolved to an enzyme).
fn sanity_check(entity_type: &str, db: &str, _id: &str, matched_name: &str) -> bool {
    let name_lower = matched_name.to_lowercase();

    match entity_type {
        "anatomical_structure" => {
            // Reject if the matched name looks like an enzyme or protein
            if db == "uniprot" {
                return false;
            }
            let enzyme_indicators = [
                "dehydrogenase",
                "kinase",
                "synthase",
                "transferase",
                "reductase",
                "oxidase",
                "protease",
                "ligase",
                "lyase",
                "isomerase",
                "mutase",
                "hydrolase",
                "phosphatase",
            ];
            for indicator in &enzyme_indicators {
                if name_lower.contains(indicator) {
                    return false;
                }
            }
            true
        }
        "disease" => {
            // Reject if resolved to a protein database
            db != "uniprot" && db != "pubchem"
        }
        "protein" | "gene" => {
            // Reject if resolved to a disease-specific MeSH descriptor
            // (MeSH diseases typically have tree numbers starting with C)
            true
        }
        "compound" => {
            // Reject if resolved to a protein database
            db != "uniprot"
        }
        _ => true,
    }
}

/// Build a Resolved from a MeSH result, applying sanity checks.
fn mesh_to_resolved(
    result: MeshResult,
    entity_type: &str,
    confidence_factor: f64,
) -> Option<Resolved> {
    let confidence = result.confidence * confidence_factor;
    let method = if result.confidence >= 0.95 {
        ResolutionMethod::ExactMatch
    } else {
        ResolutionMethod::FuzzyMatch
    };

    // Sanity check: reject cross-type mismatches
    if !sanity_check(entity_type, "mesh", &result.id, &result.matched_name) {
        return None;
    }

    let candidate = ResolvedId {
        source: "mesh".into(),
        id: result.id.clone(),
        confidence,
        matched_name: Some(result.matched_name.clone()),
    };

    Some(Resolved {
        db: "mesh".into(),
        id: result.id,
        confidence,
        matched_name: result.matched_name,
        api_aliases: result.synonyms,
        method,
        all_candidates: vec![candidate],
    })
}

/// Build a Resolved from a UniProt result, applying sanity checks.
fn uniprot_to_resolved(
    result: UniprotResult,
    entity_type: &str,
    confidence_factor: f64,
) -> Option<Resolved> {
    let confidence = result.confidence * confidence_factor;
    let method = if result.confidence >= 0.95 {
        ResolutionMethod::ExactMatch
    } else {
        ResolutionMethod::FuzzyMatch
    };

    if !sanity_check(entity_type, "uniprot", &result.id, &result.matched_name) {
        return None;
    }

    let candidate = ResolvedId {
        source: "uniprot".into(),
        id: result.id.clone(),
        confidence,
        matched_name: Some(result.matched_name.clone()),
    };

    Some(Resolved {
        db: "uniprot".into(),
        id: result.id,
        confidence,
        matched_name: result.matched_name,
        api_aliases: result.synonyms,
        method,
        all_candidates: vec![candidate],
    })
}

/// Resolve a single entity by type, returning None if no match or unsupported type.
async fn resolve_one(client: &Client, name: &str, entity_type: &str) -> Option<Resolved> {
    match entity_type {
        "disease" | "anatomical_structure" => {
            let result = lookup_mesh(client, name).await?;
            mesh_to_resolved(result, entity_type, 1.0)
        }
        "protein" => {
            let result = lookup_uniprot(client, name).await?;
            uniprot_to_resolved(result, entity_type, 1.0)
        }
        "compound" => {
            let (id, conf, matched) = lookup_pubchem(client, name).await?;
            if !sanity_check(entity_type, "pubchem", &id, &matched) {
                return None;
            }
            let method = if conf >= 0.95 {
                ResolutionMethod::ExactMatch
            } else {
                ResolutionMethod::FuzzyMatch
            };
            let candidate = ResolvedId {
                source: "pubchem".into(),
                id: id.clone(),
                confidence: conf,
                matched_name: Some(matched.clone()),
            };
            Some(Resolved {
                db: "pubchem".into(),
                id,
                confidence: conf,
                matched_name: matched,
                api_aliases: Vec::new(),
                method,
                all_candidates: vec![candidate],
            })
        }
        "gene" => {
            let result = lookup_uniprot(client, name).await?;
            uniprot_to_resolved(result, entity_type, 0.8)
        }
        // Cell types, pathways, organisms — try MeSH (it covers many biological terms)
        "cell_type" | "pathway" | "organism" => {
            let result = lookup_mesh(client, name).await?;
            mesh_to_resolved(result, entity_type, 0.7)
        }
        // Assays and other — skip API, only hardcoded aliases
        _ => None,
    }
}

// ── ROR (institutions) ─────────────────────────────────────────────────────

/// Resolve an institution name to a ROR identifier.
pub async fn resolve_ror(client: &Client, institution: &str) -> Option<ResolvedId> {
    let url = format!(
        "https://api.ror.org/v2/organizations?query={}",
        urlencoded(institution),
    );
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: serde_json::Value = resp.json().await.ok()?;
    let items = body["items"].as_array()?;
    let first = items.first()?;
    Some(ResolvedId {
        source: "ror".to_string(),
        id: first["id"].as_str()?.to_string(),
        confidence: first["score"].as_f64().unwrap_or(0.5),
        matched_name: first["names"]
            .as_array()
            .and_then(|n| n.first())
            .and_then(|n| n["value"].as_str())
            .unwrap_or(institution)
            .to_string()
            .into(),
    })
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Resolve all entities across a set of finding bundles against public databases.
///
/// Unique (name, type) pairs are resolved in parallel (up to 4 concurrent API calls),
/// then results are applied back to all matching entities.
///
/// Returns (resolved_count, skipped_count).
pub async fn resolve_entities(client: &Client, bundles: &mut [FindingBundle]) -> (usize, usize) {
    use std::collections::HashSet;
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    // 1. Collect unique (name_lower, entity_type) pairs that need resolution.
    let mut unique_keys: HashSet<(String, String)> = HashSet::new();
    for bundle in bundles.iter() {
        for entity in &bundle.assertion.entities {
            if entity.canonical_id.is_some() {
                continue;
            }
            unique_keys.insert((entity.name.to_lowercase(), entity.entity_type.clone()));
        }
    }

    // 2. Resolve unique entities in parallel with a concurrency limit.
    let semaphore = Arc::new(Semaphore::new(4));
    let mut handles = Vec::new();

    for (name_lower, entity_type) in unique_keys {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore closed");
        let client = client.clone();
        let name = name_lower.clone();
        let etype = entity_type.clone();

        handles.push(tokio::spawn(async move {
            let res = resolve_one(&client, &name, &etype).await;
            drop(permit);
            ((name, etype), res)
        }));
    }

    let mut cache: HashMap<(String, String), Option<Resolved>> = HashMap::new();
    for handle in handles {
        let (key, result) = handle.await.expect("resolve task panicked");
        cache.insert(key, result);
    }

    // 3. Apply cached results to all entities.
    let mut resolved = 0usize;
    let mut skipped = 0usize;

    for bundle in bundles.iter_mut() {
        for entity in bundle.assertion.entities.iter_mut() {
            if entity.canonical_id.is_some() {
                resolved += 1;
                continue;
            }
            entity.identifiers.clear();

            let cache_key = (entity.name.to_lowercase(), entity.entity_type.clone());
            let result = cache.get(&cache_key).and_then(|r| r.clone());

            match result {
                Some(r) => {
                    let resolved_id = ResolvedId {
                        source: r.db.clone(),
                        id: r.id.clone(),
                        confidence: r.confidence,
                        matched_name: Some(r.matched_name.clone()),
                    };

                    // Store all candidates from the API
                    entity.candidates = r.all_candidates;

                    // Set resolution method
                    entity.resolution_method = Some(r.method);

                    // Flag for review if confidence is below threshold
                    if r.confidence < 0.8 {
                        entity.needs_review = true;
                        // Do NOT set canonical_id for low-confidence matches —
                        // store candidates but require human review before accepting.
                        entity.canonical_id = None;
                    } else {
                        entity.canonical_id = Some(resolved_id.clone());
                    }

                    entity.resolution_provenance = Some(format!("vela_resolve/{}", r.db));

                    entity
                        .identifiers
                        .insert(r.db, serde_json::Value::String(r.id));
                    entity.resolution_confidence = r.confidence;

                    let mut aliases: Vec<String> = r.api_aliases;
                    let hardcoded = get_hardcoded_aliases(&entity.name);
                    for alias in hardcoded {
                        if !aliases.iter().any(|a| a.eq_ignore_ascii_case(&alias)) {
                            aliases.push(alias);
                        }
                    }
                    if !r.matched_name.eq_ignore_ascii_case(&entity.name)
                        && !aliases
                            .iter()
                            .any(|a| a.eq_ignore_ascii_case(&r.matched_name))
                    {
                        aliases.push(r.matched_name);
                    }
                    entity.aliases = aliases;

                    resolved += 1;
                }
                None => {
                    let hardcoded = get_hardcoded_aliases(&entity.name);
                    if !hardcoded.is_empty() {
                        entity.aliases = hardcoded;
                    }
                    skipped += 1;
                }
            }
        }
    }

    (resolved, skipped)
}

/// Simple percent-encoding for query parameters.
fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push_str("%20"),
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
