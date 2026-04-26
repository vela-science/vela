//! # Legacy LLM-driven link inference (moved from `vela-protocol::link` in v0.27)
//!
//! Reads a slice of `FindingBundle`s, sends batched summaries to
//! the legacy raw-API LLM client, asks for typed relationships
//! (supports / contradicts / extends / depends / replicates /
//! supersedes), and writes the inferred links back onto the
//! bundles. Dedups against any deterministic links already
//! present from `vela_protocol::link::deterministic_links`.
//!
//! Doctrine: this module lives in `vela-scientist` so the substrate
//! has zero LLM dependency. The deterministic entity-overlap
//! pass (`deterministic_links`) stays in the substrate — it's
//! pure data.

use std::collections::HashSet;

use reqwest::Client;
use vela_protocol::bundle::{FindingBundle, VALID_LINK_TYPES};

use crate::legacy_llm::{self, LlmConfig};

/// Maximum number of findings to send to the LLM in a single
/// link-inference batch.
const LLM_LINK_BATCH_SIZE: usize = 20;

const LINK_PROMPT: &str = r#"You are the Vela linker. Given numbered scientific findings, infer typed relationships.

Link types:
- supports: A provides direct evidence for B
- contradicts: A provides evidence against B
- extends: A builds on B under new conditions
- depends: A requires B to be true
- replicates: A independently reproduces B's result
- supersedes: A replaces B with stronger evidence

Rules:
- Only create links with clear scientific basis.
- Prefer fewer, high-quality links over many weak ones.
- Look for: contradictions, cross-domain extensions, dependency chains.
- Return 10-20 links per batch.

Return a JSON array:
[{"from": 0, "to": 3, "type": "supports", "note": "brief explanation"}]

Return ONLY the JSON array."#;

pub async fn infer_links(
    client: &Client,
    config: &LlmConfig,
    bundles: &mut [FindingBundle],
) -> Result<usize, String> {
    if bundles.len() < 2 {
        return Ok(0);
    }

    // Snapshot existing deterministic links for dedup.
    let existing_links: HashSet<(String, String)> = bundles
        .iter()
        .flat_map(|b| b.links.iter().map(|l| (b.id.clone(), l.target.clone())))
        .collect();

    let mut total_links = 0;

    for start in (0..bundles.len()).step_by(LLM_LINK_BATCH_SIZE) {
        let end = (start + LLM_LINK_BATCH_SIZE).min(bundles.len());
        let chunk_len = end - start;

        // Build summary
        let summary: String = bundles[start..end]
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let entities: Vec<_> = b
                    .assertion
                    .entities
                    .iter()
                    .take(3)
                    .map(|e| e.name.as_str())
                    .collect();
                format!(
                    "[{}] ({:.1}) \"{}\" [{}]",
                    i,
                    b.confidence.score,
                    {
                        let text = &b.assertion.text;
                        let end = text
                            .char_indices()
                            .take(120)
                            .last()
                            .map(|(i, c)| i + c.len_utf8())
                            .unwrap_or(text.len());
                        &text[..end]
                    },
                    entities.join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Try up to 2 times
        for attempt in 0..2 {
            match legacy_llm::call(client, config, LINK_PROMPT, &summary).await {
                Ok(raw) => match legacy_llm::parse_json(&raw) {
                    Ok(parsed) => {
                        let links = match parsed {
                            serde_json::Value::Array(arr) => arr,
                            serde_json::Value::Object(map) => map
                                .into_iter()
                                .find_map(|(_, v)| {
                                    if let serde_json::Value::Array(a) = v {
                                        Some(a)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_default(),
                            _ => Vec::new(),
                        };

                        let mut pending: Vec<(usize, String, String, String)> = Vec::new();
                        for rl in &links {
                            let fi = rl["from"].as_u64().unwrap_or(0) as usize;
                            let ti = rl["to"].as_u64().unwrap_or(1) as usize;
                            let lt = rl["type"].as_str().unwrap_or("supports");
                            let note = rl["note"].as_str().unwrap_or("");

                            if fi < chunk_len
                                && ti < chunk_len
                                && fi != ti
                                && VALID_LINK_TYPES.contains(&lt)
                            {
                                let from_id = bundles[start + fi].id.clone();
                                let target_id = bundles[start + ti].id.clone();

                                // Dedup: if a deterministic link already connects these,
                                // merge by updating the existing link's type (LLM has more
                                // context) and appending entity-overlap note.
                                if existing_links.contains(&(from_id.clone(), target_id.clone())) {
                                    if let Some(existing) =
                                        bundles[start + fi].links.iter_mut().find(|l| {
                                            l.target == target_id && l.inferred_by == "compiler"
                                        })
                                    {
                                        let entity_note = existing.note.clone();
                                        existing.link_type = lt.to_string();
                                        existing.note =
                                            format!("{} [entity_overlap: {}]", note, entity_note);
                                        existing.inferred_by = "compiler".to_string();
                                        total_links += 1; // count the merge
                                    }
                                } else {
                                    pending.push((
                                        start + fi,
                                        target_id,
                                        lt.to_string(),
                                        note.to_string(),
                                    ));
                                }
                            }
                        }

                        for (fi, target_id, lt, note) in pending {
                            bundles[fi].add_link(&target_id, &lt, &note);
                            total_links += 1;
                        }

                        break;
                    }
                    Err(_) if attempt == 0 => continue,
                    Err(e) => {
                        eprintln!("link inference parse failed (final attempt): {e}");
                        break;
                    }
                },
                Err(_) if attempt == 0 => continue,
                Err(e) => {
                    eprintln!("link inference call failed (final attempt): {e}");
                    break;
                }
            }
        }
    }

    Ok(total_links)
}
