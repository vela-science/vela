//! Frozen witness discovery for `vela reproduce`.

use std::path::{Path, PathBuf};

/// Parse either a bare witness or a record with a `witness` field.
pub(crate) fn parse_witness(raw: &str) -> Result<vela_verify::Witness, String> {
    if let Ok(witness) = serde_json::from_str::<vela_verify::Witness>(raw) {
        return Ok(witness);
    }
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    value
        .get("witness")
        .cloned()
        .ok_or_else(|| "not a witness (missing recognized `kind` and `witness`)".to_string())
        .and_then(|witness| serde_json::from_value(witness).map_err(|error| error.to_string()))
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
