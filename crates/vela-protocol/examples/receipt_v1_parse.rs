//! Batch adapter used by the parent cross-implementation Receipt v1 gate.
//!
//! This is deliberately parse-only: it reads files, reports parser outcomes,
//! and performs no landing, signing, or custody action.

use std::path::PathBuf;

use serde_json::json;
use vela_protocol::receipt_v1::ReceiptV1;

fn main() {
    let results: Vec<_> = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .map(|path| match std::fs::read(&path) {
            Ok(bytes) => match ReceiptV1::parse(&bytes) {
                Ok(receipt) => json!({
                    "path": path,
                    "ok": true,
                    "canonical_root": receipt.canonical_root().ok(),
                }),
                Err(cause) => json!({
                    "path": path,
                    "ok": false,
                    "error": cause.to_string(),
                }),
            },
            Err(cause) => json!({
                "path": path,
                "ok": false,
                "error": format!("read failed: {cause}"),
            }),
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string(&results).expect("serialize parser results")
    );
}
