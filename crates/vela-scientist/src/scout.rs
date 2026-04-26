//! # Literature Scout
//!
//! v0.22 entry point: walks a folder of PDFs, extracts candidate
//! findings, and writes them as `finding.add` `StateProposal`s into
//! the target frontier. Each proposal carries an `AgentRun` with the
//! scout's run id so the Workbench Inbox can group them and the
//! reviewer can see what model produced what claim.
//!
//! The extraction logic itself isn't here yet — that's Day 2.
//! This module defines the public boundary so the protocol crate,
//! the CLI, and the Workbench can all depend on a stable shape
//! while the inside fills in.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vela_protocol::proposals::AgentRun;

/// Inputs to a single Literature Scout run.
#[derive(Debug, Clone)]
pub struct ScoutInput {
    /// Folder to walk. PDFs are picked up; other file types are
    /// ignored in v0.22 (Notes Compiler v0.23 covers Markdown).
    pub folder: PathBuf,
    /// Path to the frontier file the proposals will be written into.
    /// The scout never modifies findings — only the `proposals` array.
    pub frontier_path: PathBuf,
    /// Optional model identifier override. When `None`, the runtime
    /// reads from environment (`VELA_SCIENTIST_MODEL` etc.).
    pub model: Option<String>,
    /// When `true`, the scout writes proposals to disk. When `false`,
    /// it returns the report without persisting — useful for dry-run
    /// previews in the CLI.
    pub apply: bool,
}

/// One proposed finding-add, ready to be wrapped in a
/// `StateProposal` and appended to the frontier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoutCandidate {
    /// The PDF this candidate was extracted from, relative to the
    /// scout's input folder.
    pub source_file: String,
    /// Human-readable claim text the reviewer will see first.
    pub claim: String,
    /// One or more source spans pinning the claim to the PDF.
    pub spans: Vec<SourceSpan>,
    /// Why the scout thinks this is a finding. Reviewer-facing.
    pub rationale: String,
    /// Coarse confidence flags surfaced as Inbox chips, not as a
    /// numeric score. Possible values: "complete", "needs_scope",
    /// "needs_evidence", "possible_duplicate", "low_confidence".
    pub flags: Vec<String>,
}

/// A pointer back into the source PDF. Page + paragraph + verbatim
/// snippet is enough for v0.22; richer span types (figure, table,
/// supplement) land later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    pub page: u32,
    pub paragraph: u32,
    pub snippet: String,
}

/// Summary returned to the CLI / Workbench.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoutReport {
    pub run: AgentRun,
    pub pdfs_seen: usize,
    pub pdfs_processed: usize,
    pub candidates_emitted: usize,
    pub proposals_written: usize,
    pub skipped: Vec<SkippedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedFile {
    pub path: String,
    pub reason: String,
}

/// Public entry point. Day 2 fills this in.
pub async fn run(_input: ScoutInput) -> Result<ScoutReport, String> {
    Err("Literature Scout extraction lands in v0.22 Day 2.".to_string())
}

/// Helper: discover PDF files in a folder, ignoring hidden and
/// non-PDF entries. Pulled out so the CLI can preview discovery
/// without invoking the model.
pub fn discover_pdfs(folder: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(folder)
        .map_err(|e| format!("read {}: {e}", folder.display()))?;
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_hidden = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        if is_hidden {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("pdf") {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}
