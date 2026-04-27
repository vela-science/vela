//! Cryptographic signing for finding bundles — the trust infrastructure layer.
//!
//! Every finding event can be signed with Ed25519 and verified independently.
//! Signatures cover the canonical JSON of the finding (deterministic, sorted keys).

use std::collections::BTreeMap;
use std::path::Path;

use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::bundle::FindingBundle;
use crate::project::Project;
use crate::repo;

/// A signed envelope wrapping a finding's cryptographic signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedEnvelope {
    pub finding_id: String,
    /// Hex-encoded Ed25519 signature (128 hex chars = 64 bytes).
    pub signature: String,
    /// Hex-encoded public key of the signer (64 hex chars = 32 bytes).
    pub public_key: String,
    /// ISO 8601 timestamp of when the signature was produced.
    pub signed_at: String,
    /// Algorithm identifier (always "ed25519").
    pub algorithm: String,
}

/// Phase M (v0.4): registered actor identity. Maps a stable `actor.id`
/// to an Ed25519 public key, established at a specific timestamp.
///
/// Once an actor is registered in a frontier, any canonical event
/// whose `actor.id` matches must carry a verifiable signature under
/// `--strict`. Frontiers without registered actors retain the legacy
/// "placeholder reviewer" rejection from v0.3 only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorRecord {
    /// Stable, namespaced identifier (e.g. "reviewer:will-blair").
    pub id: String,
    /// Hex-encoded Ed25519 public key (64 hex chars = 32 bytes).
    pub public_key: String,
    /// Algorithm identifier (always "ed25519").
    #[serde(default = "default_algorithm")]
    pub algorithm: String,
    /// ISO 8601 timestamp of when the actor was registered.
    pub created_at: String,
    /// Phase α (v0.6): trust tier permitting one-call auto-apply for a
    /// restricted set of low-risk proposal kinds. The only tier defined
    /// in v0.6 is `"auto-notes"`, which permits `propose_and_apply_note`.
    /// Tier is never honored for state-changing kinds (review, retract,
    /// confidence_revise, caveated). Pre-v0.6 actors load with `None`
    /// and behave exactly as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
}

fn default_algorithm() -> String {
    "ed25519".to_string()
}

/// Phase α (v0.6): authorization predicate for one-call auto-apply.
///
/// Returns `true` iff the actor's tier explicitly permits auto-applying
/// the given event kind without prior human review. Doctrine: tier
/// permits review-context kinds only (annotations); never state-changing
/// kinds (review verdicts, retractions, confidence revisions). Adding
/// state-changing auto-apply requires a broader tier model with
/// explicit doctrine review.
///
/// Currently recognized:
///   - `tier="auto-notes"` + `kind="finding.note"` → `true`
///   - everything else → `false`
#[must_use]
pub fn actor_can_auto_apply(actor: &ActorRecord, kind: &str) -> bool {
    matches!(
        (actor.tier.as_deref(), kind),
        (Some("auto-notes"), "finding.note")
    )
}

/// Result of verifying all signatures in a frontier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyReport {
    pub total_findings: usize,
    pub signed: usize,
    pub unsigned: usize,
    pub valid: usize,
    pub invalid: usize,
    pub signers: Vec<String>,
}

// ── Key generation ───────────────────────────────────────────────────

/// Generate an Ed25519 keypair. Writes the private key to `output_dir/private.key`
/// and the public key to `output_dir/public.key`. Both are hex-encoded.
pub fn generate_keypair(output_dir: &Path) -> Result<String, String> {
    use rand::rngs::OsRng;

    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output directory: {e}"))?;

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    let private_hex = hex::encode(signing_key.to_bytes());
    let public_hex = hex::encode(verifying_key.to_bytes());

    let private_path = output_dir.join("private.key");
    let public_path = output_dir.join("public.key");

    std::fs::write(&private_path, &private_hex)
        .map_err(|e| format!("Failed to write private key: {e}"))?;
    std::fs::write(&public_path, &public_hex)
        .map_err(|e| format!("Failed to write public key: {e}"))?;

    Ok(public_hex)
}

// ── Canonical JSON ───────────────────────────────────────────────────

/// Produce deterministic canonical JSON for a finding bundle.
/// Uses sorted keys (via serde_json::Value -> BTreeMap conversion) and compact format.
pub fn canonical_json(finding: &FindingBundle) -> Result<String, String> {
    let value =
        serde_json::to_value(finding).map_err(|e| format!("Failed to serialize finding: {e}"))?;
    let sorted = sort_value(&value);
    serde_json::to_string(&sorted).map_err(|e| format!("Failed to produce canonical JSON: {e}"))
}

/// Recursively sort all object keys in a JSON value.
fn sort_value(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let sorted: BTreeMap<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), sort_value(v)))
                .collect();
            serde_json::to_value(sorted).unwrap()
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sort_value).collect())
        }
        other => other.clone(),
    }
}

// ── Signing and verification ─────────────────────────────────────────

/// Load a signing key from a hex-encoded file.
fn load_signing_key(path: &Path) -> Result<SigningKey, String> {
    let hex_str =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read private key: {e}"))?;
    let bytes =
        hex::decode(hex_str.trim()).map_err(|e| format!("Invalid hex in private key: {e}"))?;
    let key_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "Private key must be exactly 32 bytes".to_string())?;
    Ok(SigningKey::from_bytes(&key_bytes))
}

/// Load a verifying key from a hex-encoded file.
fn load_verifying_key(path: &Path) -> Result<VerifyingKey, String> {
    let hex_str =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read public key: {e}"))?;
    parse_verifying_key(hex_str.trim())
}

/// Parse a verifying key from a hex string.
fn parse_verifying_key(hex_str: &str) -> Result<VerifyingKey, String> {
    let bytes = hex::decode(hex_str).map_err(|e| format!("Invalid hex in public key: {e}"))?;
    let key_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "Public key must be exactly 32 bytes".to_string())?;
    VerifyingKey::from_bytes(&key_bytes).map_err(|e| format!("Invalid public key: {e}"))
}

/// Sign a single finding bundle, producing a SignedEnvelope.
pub fn sign_finding(
    finding: &FindingBundle,
    signing_key: &SigningKey,
) -> Result<SignedEnvelope, String> {
    let canonical = canonical_json(finding)?;
    let signature = signing_key.sign(canonical.as_bytes());
    let public_key = signing_key.verifying_key();

    Ok(SignedEnvelope {
        finding_id: finding.id.clone(),
        signature: hex::encode(signature.to_bytes()),
        public_key: hex::encode(public_key.to_bytes()),
        signed_at: Utc::now().to_rfc3339(),
        algorithm: "ed25519".to_string(),
    })
}

/// Verify a signed envelope against a finding bundle.
pub fn verify_finding(finding: &FindingBundle, envelope: &SignedEnvelope) -> Result<bool, String> {
    if finding.id != envelope.finding_id {
        return Ok(false);
    }

    let verifying_key = parse_verifying_key(&envelope.public_key)?;
    let sig_bytes =
        hex::decode(&envelope.signature).map_err(|e| format!("Invalid signature hex: {e}"))?;
    let signature = ed25519_dalek::Signature::from_bytes(
        &sig_bytes
            .try_into()
            .map_err(|_| "Signature must be 64 bytes")?,
    );

    let canonical = canonical_json(finding)?;
    Ok(verifying_key
        .verify(canonical.as_bytes(), &signature)
        .is_ok())
}

/// Verify a finding against a specific public key (hex-encoded).
#[allow(dead_code)]
pub fn verify_finding_with_pubkey(
    finding: &FindingBundle,
    envelope: &SignedEnvelope,
    expected_pubkey: &str,
) -> Result<bool, String> {
    if envelope.public_key != expected_pubkey {
        return Ok(false);
    }
    verify_finding(finding, envelope)
}

// ── Event signing (Phase M, v0.4) ────────────────────────────────────

/// Compute the canonical signing bytes for a `StateEvent`. The `signature`
/// field is excluded from the preimage (you can't sign over your own
/// signature). The same canonical-JSON rule that derives `vev_…` is reused.
///
/// A second implementation must produce byte-identical signing bytes
/// for the same event content; the verification rule depends on it.
pub fn event_signing_bytes(event: &crate::events::StateEvent) -> Result<Vec<u8>, String> {
    use serde_json::json;
    let preimage = json!({
        "schema": event.schema,
        "id": event.id,
        "kind": event.kind,
        "target": event.target,
        "actor": event.actor,
        "timestamp": event.timestamp,
        "reason": event.reason,
        "before_hash": event.before_hash,
        "after_hash": event.after_hash,
        "payload": event.payload,
        "caveats": event.caveats,
    });
    crate::canonical::to_canonical_bytes(&preimage)
}

/// Sign a canonical event with an Ed25519 private key, returning a
/// hex-encoded signature suitable for `event.signature`.
pub fn sign_event(
    event: &crate::events::StateEvent,
    signing_key: &SigningKey,
) -> Result<String, String> {
    let bytes = event_signing_bytes(event)?;
    let signature = signing_key.sign(&bytes);
    Ok(hex::encode(signature.to_bytes()))
}

/// Verify that `event.signature` is a valid Ed25519 signature over the
/// canonical signing bytes of `event`, produced by the holder of the
/// private key matching `expected_pubkey_hex`.
pub fn verify_event_signature(
    event: &crate::events::StateEvent,
    expected_pubkey_hex: &str,
) -> Result<bool, String> {
    let signature_hex = event
        .signature
        .as_deref()
        .ok_or_else(|| format!("event {} has no signature field", event.id))?;
    let verifying_key = parse_verifying_key(expected_pubkey_hex)?;
    let sig_bytes =
        hex::decode(signature_hex).map_err(|e| format!("invalid signature hex: {e}"))?;
    let signature = ed25519_dalek::Signature::from_bytes(
        &sig_bytes
            .try_into()
            .map_err(|_| "Signature must be 64 bytes")?,
    );
    let bytes = event_signing_bytes(event)?;
    Ok(verifying_key.verify(&bytes, &signature).is_ok())
}

// ── Proposal signing (Phase Q-w, v0.5) ───────────────────────────────

/// Compute the canonical signing bytes for a `StateProposal`. The
/// `signature` (held externally on the wire) is excluded from the
/// preimage. Same canonical-JSON discipline as `event_signing_bytes`.
///
/// The proposal `id` is included, which deterministically pins the
/// content (since `vpr_…` is content-addressed under Phase P).
pub fn proposal_signing_bytes(
    proposal: &crate::proposals::StateProposal,
) -> Result<Vec<u8>, String> {
    use serde_json::json;
    let preimage = json!({
        "schema": proposal.schema,
        "id": proposal.id,
        "kind": proposal.kind,
        "target": proposal.target,
        "actor": proposal.actor,
        "created_at": proposal.created_at,
        "reason": proposal.reason,
        "payload": proposal.payload,
        "source_refs": proposal.source_refs,
        "caveats": proposal.caveats,
    });
    crate::canonical::to_canonical_bytes(&preimage)
}

/// Sign a proposal with an Ed25519 private key, returning a hex-encoded
/// signature suitable for transport on a write API.
pub fn sign_proposal(
    proposal: &crate::proposals::StateProposal,
    signing_key: &SigningKey,
) -> Result<String, String> {
    let bytes = proposal_signing_bytes(proposal)?;
    Ok(hex::encode(signing_key.sign(&bytes).to_bytes()))
}

/// Verify a hex-encoded Ed25519 signature against the canonical signing
/// bytes of `proposal`, using `expected_pubkey_hex` as the verifying key.
pub fn verify_proposal_signature(
    proposal: &crate::proposals::StateProposal,
    signature_hex: &str,
    expected_pubkey_hex: &str,
) -> Result<bool, String> {
    let verifying_key = parse_verifying_key(expected_pubkey_hex)?;
    let sig_bytes =
        hex::decode(signature_hex).map_err(|e| format!("invalid signature hex: {e}"))?;
    let signature = ed25519_dalek::Signature::from_bytes(
        &sig_bytes
            .try_into()
            .map_err(|_| "Signature must be 64 bytes")?,
    );
    let bytes = proposal_signing_bytes(proposal)?;
    Ok(verifying_key.verify(&bytes, &signature).is_ok())
}

/// Generic signature verifier for action-on-canonical-bytes: verify
/// `signature_hex` is a valid Ed25519 signature over `signing_bytes`,
/// produced by the holder of `expected_pubkey_hex`. Used by write
/// actions that don't sign over a full proposal/event struct (e.g.,
/// accept/reject decisions).
pub fn verify_action_signature(
    signing_bytes: &[u8],
    signature_hex: &str,
    expected_pubkey_hex: &str,
) -> Result<bool, String> {
    let verifying_key = parse_verifying_key(expected_pubkey_hex)?;
    let sig_bytes =
        hex::decode(signature_hex).map_err(|e| format!("invalid signature hex: {e}"))?;
    let signature = ed25519_dalek::Signature::from_bytes(
        &sig_bytes
            .try_into()
            .map_err(|_| "Signature must be 64 bytes")?,
    );
    Ok(verifying_key.verify(signing_bytes, &signature).is_ok())
}

// ── Project-level operations ────────────────────────────────────────

/// Sign all unsigned findings in a frontier. Returns the number of newly signed findings.
pub fn sign_frontier(frontier_path: &Path, private_key_path: &Path) -> Result<usize, String> {
    let mut frontier: Project = repo::load_from_path(frontier_path)?;

    let signing_key = load_signing_key(private_key_path)?;

    let mut signed_count = 0usize;

    // Build a set of already-signed finding IDs
    let already_signed: std::collections::HashSet<String> = frontier
        .signatures
        .iter()
        .map(|s| s.finding_id.clone())
        .collect();

    for finding in &frontier.findings {
        if already_signed.contains(&finding.id) {
            continue;
        }
        let envelope = sign_finding(finding, &signing_key)?;
        frontier.signatures.push(envelope);
        signed_count += 1;
    }

    repo::save_to_path(frontier_path, &frontier)?;

    Ok(signed_count)
}

/// Verify all signatures in a frontier. Optionally filter by a specific public key.
pub fn verify_frontier(
    frontier_path: &Path,
    pubkey_path: Option<&Path>,
) -> Result<VerifyReport, String> {
    let frontier: Project = repo::load_from_path(frontier_path)?;

    verify_frontier_data(&frontier, pubkey_path)
}

/// Verify all signatures in an in-memory frontier.
pub fn verify_frontier_data(
    frontier: &Project,
    pubkey_path: Option<&Path>,
) -> Result<VerifyReport, String> {
    let expected_pubkey = match pubkey_path {
        Some(path) => {
            let key = load_verifying_key(path)?;
            Some(hex::encode(key.to_bytes()))
        }
        None => None,
    };

    // Index findings by ID
    let finding_map: std::collections::HashMap<&str, &FindingBundle> = frontier
        .findings
        .iter()
        .map(|f| (f.id.as_str(), f))
        .collect();

    // Index signatures by finding ID
    let sig_map: std::collections::HashMap<&str, &SignedEnvelope> = frontier
        .signatures
        .iter()
        .map(|s| (s.finding_id.as_str(), s))
        .collect();

    let mut valid = 0usize;
    let mut invalid = 0usize;
    let mut unsigned = 0usize;
    let mut signers: std::collections::HashSet<String> = std::collections::HashSet::new();

    for finding in &frontier.findings {
        match sig_map.get(finding.id.as_str()) {
            None => {
                unsigned += 1;
            }
            Some(envelope) => {
                // If filtering by pubkey, check it matches
                if let Some(ref expected) = expected_pubkey
                    && &envelope.public_key != expected
                {
                    invalid += 1;
                    continue;
                }

                match verify_finding(finding, envelope) {
                    Ok(true) => {
                        valid += 1;
                        signers.insert(envelope.public_key.clone());
                    }
                    _ => {
                        invalid += 1;
                    }
                }
            }
        }
    }

    let _ = finding_map; // used for indexing above

    Ok(VerifyReport {
        total_findings: frontier.findings.len(),
        signed: valid + invalid,
        unsigned,
        valid,
        invalid,
        signers: signers.into_iter().collect(),
    })
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::*;

    fn sample_finding() -> FindingBundle {
        FindingBundle::new(
            Assertion {
                text: "NLRP3 activates IL-1B".into(),
                assertion_type: "mechanism".into(),
                entities: vec![Entity {
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
                }],
                relation: Some("activates".into()),
                direction: Some("positive".into()),
            },
            Evidence {
                evidence_type: "experimental".into(),
                model_system: "mouse".into(),
                species: Some("Mus musculus".into()),
                method: "Western blot".into(),
                sample_size: Some("n=30".into()),
                effect_size: None,
                p_value: Some("p<0.05".into()),
                replicated: true,
                replication_count: Some(3),
                evidence_spans: vec![],
            },
            Conditions {
                text: "In vitro, mouse microglia".into(),
                species_verified: vec!["Mus musculus".into()],
                species_unverified: vec![],
                in_vitro: true,
                in_vivo: false,
                human_data: false,
                clinical_trial: false,
                concentration_range: None,
                duration: None,
                age_group: None,
                cell_type: Some("microglia".into()),
            },
            Confidence::raw(0.85, "Experimental with replication", 0.9),
            Provenance {
                source_type: "published_paper".into(),
                doi: Some("10.1234/test".into()),
                pmid: None,
                pmc: None,
                openalex_id: None,
                url: None,
                title: "Test Paper".into(),
                authors: vec![Author {
                    name: "Smith J".into(),
                    orcid: None,
                }],
                year: Some(2024),
                journal: Some("Nature".into()),
                license: None,
                publisher: None,
                funders: vec![],
                extraction: Extraction::default(),
                review: None,
                citation_count: Some(100),
            },
            Flags {
                gap: false,
                negative_space: false,
                contested: false,
                retracted: false,
                declining: false,
                gravity_well: false,
                review_state: None,
                superseded: false,
            },
        )
    }

    fn test_keypair() -> SigningKey {
        use rand::rngs::OsRng;
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    fn keygen_produces_valid_files() {
        let dir = std::env::temp_dir().join("vela_test_keygen");
        let _ = std::fs::remove_dir_all(&dir);

        let pubkey = generate_keypair(&dir).unwrap();
        assert_eq!(pubkey.len(), 64); // 32 bytes hex-encoded

        let private_hex = std::fs::read_to_string(dir.join("private.key")).unwrap();
        let public_hex = std::fs::read_to_string(dir.join("public.key")).unwrap();
        assert_eq!(private_hex.len(), 64);
        assert_eq!(public_hex, pubkey);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let finding = sample_finding();
        let key = test_keypair();

        let envelope = sign_finding(&finding, &key).unwrap();
        assert_eq!(envelope.finding_id, finding.id);
        assert_eq!(envelope.algorithm, "ed25519");
        assert_eq!(envelope.signature.len(), 128); // 64 bytes hex-encoded

        let valid = verify_finding(&finding, &envelope).unwrap();
        assert!(valid, "Signature should verify against original finding");
    }

    #[test]
    fn tampered_finding_fails_verification() {
        let finding = sample_finding();
        let key = test_keypair();
        let envelope = sign_finding(&finding, &key).unwrap();

        // Tamper with the finding
        let mut tampered = finding.clone();
        tampered.assertion.text = "Tampered assertion text".into();

        let valid = verify_finding(&tampered, &envelope).unwrap();
        assert!(!valid, "Tampered finding should fail verification");
    }

    #[test]
    fn wrong_key_fails_verification() {
        let finding = sample_finding();
        let key1 = test_keypair();
        let key2 = test_keypair();

        let envelope = sign_finding(&finding, &key1).unwrap();
        let pubkey2_hex = hex::encode(key2.verifying_key().to_bytes());

        let valid = verify_finding_with_pubkey(&finding, &envelope, &pubkey2_hex).unwrap();
        assert!(!valid, "Wrong public key should fail verification");
    }

    #[test]
    fn canonical_json_is_deterministic() {
        let finding = sample_finding();
        let json1 = canonical_json(&finding).unwrap();
        let json2 = canonical_json(&finding).unwrap();
        assert_eq!(json1, json2, "Canonical JSON must be deterministic");
    }

    #[test]
    fn registered_actor_signed_event_roundtrip() {
        // Phase M: a registered actor's event must sign-and-verify
        // against its registered pubkey via `event_signing_bytes`. This
        // is the load-bearing claim for the v0.4 strict-mode gate.
        use crate::events::{
            EVENT_SCHEMA, NULL_HASH, StateActor, StateEvent, StateTarget, compute_event_id,
        };

        let key = test_keypair();
        let pubkey_hex = hex::encode(key.verifying_key().to_bytes());

        let mut event = StateEvent {
            schema: EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: "finding.reviewed".to_string(),
            target: StateTarget {
                r#type: "finding".to_string(),
                id: "vf_test".to_string(),
            },
            actor: StateActor {
                id: "reviewer:registered".to_string(),
                r#type: "human".to_string(),
            },
            timestamp: "2026-04-25T00:00:00Z".to_string(),
            reason: "phase-m round-trip test".to_string(),
            before_hash: NULL_HASH.to_string(),
            after_hash: "sha256:abc".to_string(),
            payload: serde_json::json!({"status": "accepted", "proposal_id": "vpr_test"}),
            caveats: vec![],
            signature: None,
        };
        event.id = compute_event_id(&event);
        event.signature = Some(sign_event(&event, &key).unwrap());

        // Verifies against the registered pubkey.
        assert!(verify_event_signature(&event, &pubkey_hex).unwrap());

        // Tampering with the reason invalidates the signature.
        let mut tampered = event.clone();
        tampered.reason = "different reason".to_string();
        assert!(!verify_event_signature(&tampered, &pubkey_hex).unwrap());
    }

    #[test]
    fn verify_frontier_data_reports_correctly() {
        let f1 = sample_finding();
        let mut f2 = sample_finding();
        f2.id = "vf_other_id_12345".into();
        f2.assertion.text = "Different finding".into();

        let key = test_keypair();
        let env1 = sign_finding(&f1, &key).unwrap();
        // Leave f2 unsigned

        let frontier = Project {
            vela_version: "0.1.0".into(),
            schema: "test".into(),
            frontier_id: None,
            project: crate::project::ProjectMeta {
                name: "test".into(),
                description: "test".into(),
                compiled_at: "2024-01-01T00:00:00Z".into(),
                compiler: "vela/0.2.0".into(),
                papers_processed: 0,
                errors: 0,
                dependencies: Vec::new(),
            },
            stats: crate::project::ProjectStats {
                findings: 2,
                links: 0,
                replicated: 0,
                unreplicated: 2,
                avg_confidence: 0.85,
                gaps: 0,
                negative_space: 0,
                contested: 0,
                categories: std::collections::HashMap::new(),
                link_types: std::collections::HashMap::new(),
                human_reviewed: 0,
                review_event_count: 0,
                confidence_update_count: 0,
                event_count: 0,
                source_count: 0,
                evidence_atom_count: 0,
                condition_record_count: 0,
                proposal_count: 0,
                confidence_distribution: crate::project::ConfidenceDistribution {
                    high_gt_80: 2,
                    medium_60_80: 0,
                    low_lt_60: 0,
                },
            },
            findings: vec![f1, f2],
            sources: vec![],
            evidence_atoms: vec![],
            condition_records: vec![],
            review_events: vec![],
            confidence_updates: vec![],
            events: vec![],
            proposals: vec![],
            proof_state: Default::default(),
            signatures: vec![env1],
            actors: vec![],
            replications: vec![],
            datasets: vec![],
            code_artifacts: vec![],
            predictions: vec![],
            resolutions: vec![],
        };

        let report = verify_frontier_data(&frontier, None).unwrap();
        assert_eq!(report.total_findings, 2);
        assert_eq!(report.signed, 1);
        assert_eq!(report.unsigned, 1);
        assert_eq!(report.valid, 1);
        assert_eq!(report.invalid, 0);
        assert_eq!(report.signers.len(), 1);
    }
}
