//! Receipt-v1 read layer: typed, parse-only views over the layered
//! `vela.receipt.v1` JSON (docs/schemas/vela.receipt.v1.schema.json).
//!
//! The rich receipt is emitted by producers and validated against the JSON
//! Schema; the Rust side has treated everything past the minimal landing
//! subset as opaque passthrough. This module types the three layers the
//! substrate now needs to READ — the graded `acceptance_scope`, the
//! `lineage` layer, and `environment.independence_basis` — without owning
//! the receipt's serialization:
//!
//! - accessors take a `&serde_json::Value` and never mutate it;
//! - nothing here is written back into stored objects or enters any
//!   content-address preimage;
//! - the layers this module does NOT type (distillation, contributors,
//!   signature identities, the DSSE envelope, status events) stay opaque
//!   on purpose — the schema and the Python emitter own their shape.
//!
//! Vocabulary note: `AcceptanceScope` (8-valued, the acceptance layer) is a
//! different axis from the gate's `GateStatus` (3-valued, derived from
//! verifier attachments) and from the status-event ladder (`status.kind`).
//! They are layers, not one enum; see docs/RECEIPTS.md ("Status
//! vocabularies") for the projection between them.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The graded acceptance scope from the receipt's acceptance layer —
/// what kind of standing the acceptor granted, never who granted it or
/// whether a verifier passed (those are separate fields and separate
/// trust dimensions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceScope {
    MachineVerified,
    HumanSeen,
    LocallyAccepted,
    FrontierAccepted,
    CanonAccepted,
    HypothesisOnly,
    Retracted,
    Superseded,
}

impl AcceptanceScope {
    /// Every variant, in schema order. The schema-sync test below keeps
    /// this list byte-identical to `$defs/acceptance_scope.enum`.
    pub const ALL: [AcceptanceScope; 8] = [
        AcceptanceScope::MachineVerified,
        AcceptanceScope::HumanSeen,
        AcceptanceScope::LocallyAccepted,
        AcceptanceScope::FrontierAccepted,
        AcceptanceScope::CanonAccepted,
        AcceptanceScope::HypothesisOnly,
        AcceptanceScope::Retracted,
        AcceptanceScope::Superseded,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            AcceptanceScope::MachineVerified => "machine_verified",
            AcceptanceScope::HumanSeen => "human_seen",
            AcceptanceScope::LocallyAccepted => "locally_accepted",
            AcceptanceScope::FrontierAccepted => "frontier_accepted",
            AcceptanceScope::CanonAccepted => "canon_accepted",
            AcceptanceScope::HypothesisOnly => "hypothesis_only",
            AcceptanceScope::Retracted => "retracted",
            AcceptanceScope::Superseded => "superseded",
        }
    }

    pub fn parse(s: &str) -> Option<AcceptanceScope> {
        AcceptanceScope::ALL
            .iter()
            .copied()
            .find(|v| v.as_str() == s)
    }
}

/// The receipt's top-level `lineage` layer: where this work came from.
/// The schema requires `parents`/`derived_from`/`source_refs` and allows
/// more; unknown fields are ignored on read.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReceiptLineage {
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(default)]
    pub derived_from: Vec<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub supersedes: Vec<String>,
    #[serde(default)]
    pub producer_run_id: Option<String>,
    #[serde(default)]
    pub frontier: Option<String>,
}

/// `environment.independence_basis`: the producer-declared basis on which
/// two evaluations could be judged independent. Inspectable and refutable,
/// never an assertion of independence itself — the derived predicate in
/// `analysis::independence` is what turns this into a judgment, and the
/// gate's attachment-level `independent_of` remains the enforced
/// counterpart.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IndependenceBasis {
    #[serde(default)]
    pub method_family: String,
    #[serde(default)]
    pub solver_identity: String,
    #[serde(default)]
    pub code_lineage: String,
    #[serde(default)]
    pub dataset_lineage: String,
    #[serde(default)]
    pub model_lineage: String,
    #[serde(default)]
    pub shared_dependencies: Vec<String>,
    #[serde(default)]
    pub declared_independent_of: Vec<String>,
    #[serde(default)]
    pub known_couplings: Vec<String>,
}

/// Read the `lineage` layer from a full receipt value. `None` when the
/// layer is absent or not an object — an old or minimal receipt, which is
/// legal; missing lineage must never be mistaken for clean lineage.
pub fn lineage_from_receipt(receipt: &Value) -> Option<ReceiptLineage> {
    lineage_from_layer(receipt.get("lineage")?)
}

/// Read a value that IS the lineage layer (callers that hold the layer
/// directly, e.g. the landing receipt's `lineage` field).
pub fn lineage_from_layer(layer: &Value) -> Option<ReceiptLineage> {
    if !layer.is_object() {
        return None;
    }
    serde_json::from_value(layer.clone()).ok()
}

/// Read `independence_basis` from a receipt's `environment` value (pass
/// `receipt["environment"]`, not the whole receipt).
pub fn independence_basis_from_environment(environment: &Value) -> Option<IndependenceBasis> {
    let basis = environment.get("independence_basis")?;
    if !basis.is_object() {
        return None;
    }
    serde_json::from_value(basis.clone()).ok()
}

/// Read the graded `acceptance_scope` from a full receipt value
/// (`acceptance.acceptance_scope`).
pub fn acceptance_scope_from_receipt(receipt: &Value) -> Option<AcceptanceScope> {
    receipt
        .get("acceptance")?
        .get("acceptance_scope")?
        .as_str()
        .and_then(AcceptanceScope::parse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn acceptance_scope_round_trips_every_variant() {
        for v in AcceptanceScope::ALL {
            assert_eq!(AcceptanceScope::parse(v.as_str()), Some(v));
            let ser = serde_json::to_value(v).unwrap();
            assert_eq!(ser, json!(v.as_str()));
        }
        assert_eq!(AcceptanceScope::parse("verified"), None);
    }

    #[test]
    fn acceptance_scope_matches_the_shipped_schema() {
        // The schema copy in this repo is kept in lockstep with the
        // campaign copies by the receipt-schema-sync gate; this test keeps
        // the Rust enum in lockstep with the schema, so drift anywhere in
        // the chain fails a build.
        let schema_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/schemas/vela.receipt.v1.schema.json"
        );
        let raw = std::fs::read_to_string(schema_path).expect("schema file readable");
        let schema: Value = serde_json::from_str(&raw).expect("schema parses");
        let enum_values: Vec<String> = schema["$defs"]["acceptance_scope"]["enum"]
            .as_array()
            .expect("acceptance_scope enum present")
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let ours: Vec<String> = AcceptanceScope::ALL
            .iter()
            .map(|v| v.as_str().to_string())
            .collect();
        assert_eq!(ours, enum_values, "Rust enum drifted from the schema");
    }

    #[test]
    fn lineage_reads_required_and_optional_fields() {
        let receipt = json!({
            "lineage": {
                "parents": ["vrc_aaa"],
                "derived_from": ["arxiv:2406.00001"],
                "source_refs": ["https://example.org/run/1"],
                "producer_run_id": "run-17",
                "unknown_extra": {"kept": "loose"}
            }
        });
        let lineage = lineage_from_receipt(&receipt).expect("parses");
        assert_eq!(lineage.parents, vec!["vrc_aaa"]);
        assert_eq!(lineage.derived_from, vec!["arxiv:2406.00001"]);
        assert_eq!(lineage.producer_run_id.as_deref(), Some("run-17"));
        assert!(lineage.supersedes.is_empty());
        assert!(lineage.frontier.is_none());
    }

    #[test]
    fn absent_layers_read_as_none_not_default() {
        assert_eq!(lineage_from_receipt(&json!({})), None);
        assert_eq!(lineage_from_receipt(&json!({"lineage": "text"})), None);
        assert_eq!(independence_basis_from_environment(&json!({})), None);
        assert_eq!(acceptance_scope_from_receipt(&json!({})), None);
        assert_eq!(
            acceptance_scope_from_receipt(&json!({"acceptance": {"acceptance_scope": "bogus"}})),
            None
        );
    }

    #[test]
    fn independence_basis_reads_couplings() {
        let env = json!({
            "independence_basis": {
                "method_family": "sat",
                "solver_identity": "kissat-3.1",
                "known_couplings": ["model:claude-fable-5"],
                "declared_independent_of": ["vva_123"]
            }
        });
        let basis = independence_basis_from_environment(&env).expect("parses");
        assert_eq!(basis.known_couplings, vec!["model:claude-fable-5"]);
        assert_eq!(basis.declared_independent_of, vec!["vva_123"]);
        assert!(basis.code_lineage.is_empty());
    }

    #[test]
    fn acceptance_scope_reads_from_full_receipt() {
        let receipt = json!({
            "acceptance": {
                "profile": "math.exact",
                "acceptance_scope": "machine_verified"
            }
        });
        assert_eq!(
            acceptance_scope_from_receipt(&receipt),
            Some(AcceptanceScope::MachineVerified)
        );
    }
}
