//! Hub inspection and verification handlers.

use crate::cli::{cmd_verify_chain, fail, fail_return, print_json};
use crate::cli_commands::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use vela_protocol::cli_style as style;

/// Hub index inspection and transparency verification.
///
/// Source discovery is Hub operator configuration; frontier publication is a
/// normal `git push` to a configured repository.
pub(crate) async fn cmd_hub(action: HubAction) {
    match action {
        HubAction::VerifyChain {
            frontier,
            artifacts,
            json,
        } => cmd_verify_chain(frontier, artifacts, json),
        HubAction::WitnessCheck { vfr_id, hubs, json } => {
            // v0.129: A11 mitigation. Pull `vfr_id` from every named
            // hub, canonicalize each entry, compare. Reports per-hub
            // canonical hash plus consensus signal:
            //   `unanimous`  — every hub returned byte-identical
            //                   canonical bytes.
            //   `majority`   — most hubs agree; some diverge.
            //   `split`      — no hub has a majority.
            //   `insufficient` — fewer than 2 hubs responded.
            if hubs.len() < 2 {
                fail("--hubs requires at least two hub URLs (comma-separated).");
            }

            #[derive(serde::Serialize)]
            struct HubResponse {
                hub: String,
                status: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                canonical_hash: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                note: Option<String>,
            }

            // This branch runs inside the CLI's Tokio runtime. Use the native
            // async client: dropping reqwest's blocking client here panics.
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|e| fail_return(&format!("http client init: {e}")));
            let mut responses: Vec<HubResponse> = Vec::new();
            let mut hash_counts: std::collections::BTreeMap<String, usize> =
                std::collections::BTreeMap::new();

            for hub_url in &hubs {
                let base = hub_url.trim_end_matches('/');
                let url = format!("{base}/entries/{vfr_id}");
                match client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        match resp.json::<serde_json::Value>().await {
                            Ok(entry) => {
                                // The current frontier-index row contains
                                // only source-derived identity and content;
                                // compare the whole canonical object.
                                // Canonicalize via the substrate's
                                // canonical-bytes helper so hub-side
                                // key ordering or whitespace
                                // differences do not falsely split.
                                let canonical =
                                    vela_protocol::canonical::to_canonical_bytes(&entry)
                                        .unwrap_or_else(|e| {
                                            fail_return(&format!("canonicalize: {e}"))
                                        });
                                let hash =
                                    format!("sha256:{}", hex::encode(Sha256::digest(&canonical)));
                                *hash_counts.entry(hash.clone()).or_insert(0) += 1;
                                responses.push(HubResponse {
                                    hub: base.to_string(),
                                    status: "ok".to_string(),
                                    canonical_hash: Some(hash),
                                    note: None,
                                });
                            }
                            Err(e) => responses.push(HubResponse {
                                hub: base.to_string(),
                                status: "parse_error".to_string(),
                                canonical_hash: None,
                                note: Some(format!("parse: {e}")),
                            }),
                        }
                    }
                    Ok(resp) => responses.push(HubResponse {
                        hub: base.to_string(),
                        status: "http_error".to_string(),
                        canonical_hash: None,
                        note: Some(format!("HTTP {}", resp.status())),
                    }),
                    Err(e) => responses.push(HubResponse {
                        hub: base.to_string(),
                        status: "unreachable".to_string(),
                        canonical_hash: None,
                        note: Some(format!("{e}")),
                    }),
                }
            }

            // Consensus signal.
            let resolved_count = responses
                .iter()
                .filter(|r| r.canonical_hash.is_some())
                .count();
            let consensus = if resolved_count < 2 {
                "insufficient".to_string()
            } else if hash_counts.len() == 1 {
                "unanimous".to_string()
            } else {
                let max = hash_counts.values().copied().max().unwrap_or(0);
                if max * 2 > resolved_count {
                    "majority".to_string()
                } else {
                    "split".to_string()
                }
            };

            let payload = json!({
                "ok": consensus == "unanimous" || consensus == "majority",
                "command": "hub.witness-check",
                "vfr_id": vfr_id,
                "hubs_queried": hubs.len(),
                "hubs_resolved": resolved_count,
                "distinct_canonical_hashes": hash_counts.len(),
                "consensus": consensus,
                "responses": responses,
            });

            if json {
                print_json(&payload);
            } else {
                println!(
                    "{} witness-check {} across {} hub(s): {}",
                    style::ok("hub"),
                    vfr_id,
                    hubs.len(),
                    consensus
                );
                for r in &responses {
                    let hash_display = r
                        .canonical_hash
                        .as_deref()
                        .map(|h| h.chars().take(16).collect::<String>())
                        .unwrap_or_else(|| r.note.clone().unwrap_or_default());
                    println!("  {} {} {hash_display}", r.status, r.hub);
                }
            }
            if consensus == "split" {
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[tokio::test(flavor = "current_thread")]
    async fn witness_check_is_safe_inside_the_cli_runtime() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 1024];
                let _ = stream.read(&mut request).unwrap();
                let body = br#"{"state":"same"}"#;
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(body).unwrap();
            }
        });

        cmd_hub(HubAction::WitnessCheck {
            vfr_id: "vfr_runtime_regression".to_string(),
            hubs: vec![format!("http://{address}"), format!("http://{address}")],
            json: true,
        })
        .await;

        server.join().unwrap();
    }
}
