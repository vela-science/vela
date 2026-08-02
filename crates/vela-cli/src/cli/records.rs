//! Frozen witness discovery for `vela reproduce`.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ReproductionWitness {
    Vela(vela_verify::Witness),
    QuantumStabilizer(vela_verify::QuantumStabilizerWitnessV1),
}

impl ReproductionWitness {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Vela(witness) => witness.kind(),
            Self::QuantumStabilizer(_) => "quantum_stabilizer",
        }
    }

    pub(crate) fn verify(&self) -> vela_verify::VerifyResult {
        match self {
            Self::Vela(witness) => vela_verify::verify_witness(witness),
            Self::QuantumStabilizer(witness) => {
                vela_verify::verify_quantum_stabilizer_witness_v1(witness)
            }
        }
    }

    pub(crate) fn dominates(&self, prior: &Self) -> Result<bool, String> {
        match (self, prior) {
            (Self::Vela(current), Self::Vela(prior)) => vela_verify::dominates(current, prior),
            _ => Err(format!(
                "no dominance order defined between {} and {}",
                self.kind(),
                prior.kind()
            )),
        }
    }
}

/// Parse one current bare witness representation.
pub(crate) fn parse_witness(raw: &str) -> Result<ReproductionWitness, String> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    parse_witness_value(value)
}

fn parse_witness_value(value: serde_json::Value) -> Result<ReproductionWitness, String> {
    if let Ok(witness) = serde_json::from_value::<vela_verify::Witness>(value.clone()) {
        return Ok(ReproductionWitness::Vela(witness));
    }
    if value.get("schema").and_then(serde_json::Value::as_str)
        == Some("canopus.quantum-stabilizer-witness.v1")
    {
        return serde_json::from_value(value)
            .map(ReproductionWitness::QuantumStabilizer)
            .map_err(|error| error.to_string());
    }
    Err("not a recognized current witness".into())
}

/// Collect a single witness or every `*.witness.json` below a directory.
pub(crate) fn collect_witness_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    let witnesses = path.join("witnesses");
    let root = if witnesses.is_dir() { &witnesses } else { path };
    let mut files = Vec::new();
    collect_witness_files_into(root, &mut files);
    files.sort();
    files
}

fn collect_witness_files_into(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_witness_files_into(&path, files);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".witness.json"))
        {
            files.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUANTUM_WITNESS: &str = r#"{
        "schema": "canopus.quantum-stabilizer-witness.v1",
        "target": "quantum:[[10,1,4]]",
        "n": 10,
        "k": 1,
        "generators": [
            "IXIZYXYXYZ",
            "IXZXIXXIIZ",
            "XXIYYZIZXZ",
            "ZXIYZIXYXX",
            "XXIYXXXIYY",
            "YIIXYXZYYY",
            "IZXYYYZIZX",
            "ZXXZZIZZIY",
            "IIZXXXIIYZ"
        ]
    }"#;

    #[test]
    fn parses_and_verifies_the_retained_quantum_schema() {
        let witness = parse_witness(QUANTUM_WITNESS).unwrap();
        assert_eq!(witness.kind(), "quantum_stabilizer");
        let outcome = witness.verify();
        assert!(outcome.ok, "{}", outcome.message);
        assert_eq!(outcome.value, Some(4.0));
    }

    #[test]
    fn quantum_schema_still_fails_closed_on_invalid_generators() {
        let mut value: serde_json::Value = serde_json::from_str(QUANTUM_WITNESS).unwrap();
        value["generators"][8] = value["generators"][0].clone();
        let witness = parse_witness(&serde_json::to_string(&value).unwrap()).unwrap();
        let outcome = witness.verify();
        assert!(!outcome.ok);
        assert!(outcome.message.contains("distinct"));
    }

    #[test]
    fn wrapped_witness_is_not_a_current_representation() {
        let wrapped = serde_json::json!({
            "witness": serde_json::from_str::<serde_json::Value>(QUANTUM_WITNESS).unwrap()
        });
        let error = parse_witness(&serde_json::to_string(&wrapped).unwrap()).unwrap_err();
        assert_eq!(error, "not a recognized current witness");
    }
}
