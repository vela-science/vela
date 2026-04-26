//! # Literature Scout
//!
//! Walks a folder of PDFs, extracts candidate findings via the
//! shared LLM extractor in `vela-protocol::ingest`, and writes them
//! into the target frontier as `finding.add` `StateProposal`s tagged
//! with an `AgentRun`. Reviewers see them in the Workbench Inbox.
//!
//! Scope discipline (v0.22):
//! * **PDFs only.** Markdown / Obsidian is Notes Compiler in v0.23.
//! * **One model call per PDF.** No multi-pass refinement, no chunking
//!   beyond what the underlying extractor already does.
//! * **Always proposes, never applies.** Even with `apply: true`, the
//!   only state the scout writes is `frontier.proposals` — never
//!   `frontier.findings`. Acceptance happens in the Workbench.
//! * **Substrate stays dumb.** The proposal payload uses the standard
//!   `finding.add` shape; reading it requires no knowledge of the
//!   scout, the model, or the prompt. Removing this crate would
//!   leave every accepted finding intact.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use vela_protocol::bundle::FindingBundle;
use vela_protocol::events::{StateActor, StateTarget};
use vela_protocol::ingest::extract_pdf_text;
use vela_protocol::project::Project;
use vela_protocol::proposals::{AgentRun, StateProposal, new_proposal};
use vela_protocol::repo;

use crate::extract::extract_via_claude_cli;
use crate::{AGENT_ACTOR_ID_LITERATURE_SCOUT, AGENT_LITERATURE_SCOUT, new_run_id};

/// Inputs to a single Literature Scout run.
#[derive(Debug, Clone)]
pub struct ScoutInput {
    /// Folder to walk. Only `*.pdf` files at the top level are
    /// considered in v0.22; recursion lands later if the dogfood run
    /// shows it's needed.
    pub folder: PathBuf,
    /// Frontier file the proposals will be appended to.
    pub frontier_path: PathBuf,
    /// Optional model alias. Threaded through to `claude --model`.
    /// `None` lets the user's Claude Code session pick its default.
    pub model: Option<String>,
    /// Path to the `claude` CLI binary. Defaults to `"claude"` on
    /// PATH; override for tests or unusual installs.
    pub cli_command: String,
    /// When `false`, the scout reports what it would do but never
    /// writes proposals to disk. Useful for previewing on a folder
    /// before paying the user's Claude Code quota.
    pub apply: bool,
}

impl Default for ScoutInput {
    fn default() -> Self {
        Self {
            folder: PathBuf::new(),
            frontier_path: PathBuf::new(),
            model: None,
            cli_command: "claude".to_string(),
            apply: true,
        }
    }
}

/// One proposed finding-add, ready to be wrapped in a
/// `StateProposal`. Currently produced by the underlying extractor;
/// kept as an explicit type so the public boundary stays stable
/// even when we swap the extractor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoutCandidate {
    pub source_file: String,
    pub finding: FindingBundle,
    /// Why the scout produced this candidate. Reviewer-facing.
    pub rationale: String,
    /// Coarse status flags: "complete", "needs_scope", "needs_evidence",
    /// "possible_duplicate", "low_confidence". Surfaced as Inbox chips.
    pub flags: Vec<String>,
}

/// A pointer back into the source PDF. Reserved for v0.23; the v0.22
/// extractor only attaches `Evidence::evidence_spans` strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    pub page: u32,
    pub paragraph: u32,
    pub snippet: String,
}

/// Summary returned to the CLI / Workbench after a run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoutReport {
    pub run: AgentRun,
    pub pdfs_seen: usize,
    pub pdfs_processed: usize,
    pub candidates_emitted: usize,
    pub proposals_written: usize,
    pub skipped: Vec<SkippedFile>,
    pub frontier_path: String,
    pub apply: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedFile {
    pub path: String,
    pub reason: String,
}

/// Top-level entry point. Returns a `ScoutReport`; on `apply: true`
/// the frontier file is rewritten with the new proposals appended.
pub async fn run(input: ScoutInput) -> Result<ScoutReport, String> {
    let started_at = Utc::now().to_rfc3339();
    let run_id = new_run_id(AGENT_LITERATURE_SCOUT);

    let pdfs = discover_pdfs(&input.folder)?;
    let pdfs_seen = pdfs.len();

    let mut frontier: Project = repo::load_from_path(&input.frontier_path)
        .map_err(|e| format!("load frontier {}: {e}", input.frontier_path.display()))?;

    let mut report = ScoutReport {
        run: AgentRun {
            agent: AGENT_LITERATURE_SCOUT.to_string(),
            model: input.model.clone().unwrap_or_default(),
            run_id: run_id.clone(),
            started_at: started_at.clone(),
            finished_at: None,
            context: std::collections::BTreeMap::from([
                (
                    "input_folder".to_string(),
                    input.folder.display().to_string(),
                ),
                ("pdf_count".to_string(), pdfs_seen.to_string()),
                ("backend".to_string(), "claude-cli".to_string()),
                ("cli_command".to_string(), input.cli_command.clone()),
            ]),
        },
        pdfs_seen,
        pdfs_processed: 0,
        candidates_emitted: 0,
        proposals_written: 0,
        skipped: Vec::new(),
        frontier_path: input.frontier_path.display().to_string(),
        apply: input.apply,
    };

    let existing_finding_ids: std::collections::HashSet<String> = frontier
        .findings
        .iter()
        .map(|f| f.id.clone())
        .collect();
    let existing_proposal_ids: std::collections::HashSet<String> = frontier
        .proposals
        .iter()
        .map(|p| p.id.clone())
        .collect();

    let mut new_proposals: Vec<StateProposal> = Vec::new();

    for pdf in &pdfs {
        let label = pdf.display().to_string();
        let text = match extract_pdf_text(pdf) {
            Ok(t) if !t.trim().is_empty() => t,
            Ok(_) => {
                report.skipped.push(SkippedFile {
                    path: label,
                    reason: "empty PDF text after extraction".to_string(),
                });
                continue;
            }
            Err(e) => {
                report.skipped.push(SkippedFile {
                    path: label,
                    reason: format!("extract failed: {e}"),
                });
                continue;
            }
        };

        let candidates = match extract_via_claude_cli(
            &text,
            pdf,
            input.model.as_deref(),
            &input.cli_command,
        ) {
            Ok(b) => b,
            Err(e) => {
                report.skipped.push(SkippedFile {
                    path: label,
                    reason: format!("LLM extract failed: {e}"),
                });
                continue;
            }
        };

        report.pdfs_processed += 1;
        for (rationale, finding) in candidates {
            report.candidates_emitted += 1;

            // Skip duplicates the substrate would reject anyway —
            // the Workbench can fold these into a "possible
            // duplicate" surface later.
            let mut flags: Vec<String> = Vec::new();
            if existing_finding_ids.contains(&finding.id) {
                flags.push("duplicate_finding".to_string());
                report.skipped.push(SkippedFile {
                    path: format!("{}#{}", pdf.display(), finding.id),
                    reason: "finding id already in frontier".to_string(),
                });
                continue;
            }

            let proposal = build_proposal(
                &finding,
                pdf,
                &report.run,
                &flags,
                &rationale,
            );
            if existing_proposal_ids.contains(&proposal.id) {
                report.skipped.push(SkippedFile {
                    path: format!("{}#{}", pdf.display(), proposal.id),
                    reason: "proposal id already in frontier".to_string(),
                });
                continue;
            }
            new_proposals.push(proposal);
        }
    }

    if input.apply && !new_proposals.is_empty() {
        for p in new_proposals.drain(..) {
            report.proposals_written += 1;
            frontier.proposals.push(p);
        }
        repo::save_to_path(&input.frontier_path, &frontier)
            .map_err(|e| format!("save frontier: {e}"))?;
    } else {
        report.proposals_written = new_proposals.len();
    }

    report.run.finished_at = Some(Utc::now().to_rfc3339());
    Ok(report)
}

fn build_proposal(
    finding: &FindingBundle,
    pdf: &Path,
    run: &AgentRun,
    flags: &[String],
    model_rationale: &str,
) -> StateProposal {
    let payload = json!({ "finding": finding });
    let source_label = pdf.display().to_string();
    let reason = if !model_rationale.trim().is_empty() {
        // The model's own one-sentence rationale is the most useful
        // thing for a reviewer to see first; fall back to a generic
        // extraction note only when missing.
        if flags.is_empty() {
            model_rationale.to_string()
        } else {
            format!("{model_rationale} [flags: {}]", flags.join(", "))
        }
    } else if flags.is_empty() {
        format!("Literature Scout extracted from {source_label}")
    } else {
        format!(
            "Literature Scout extracted from {source_label} [flags: {}]",
            flags.join(", ")
        )
    };
    let _ = StateActor {
        // Construction below uses new_proposal, which builds the actor
        // from id/type strings. Documenting the binding for readers.
        id: AGENT_ACTOR_ID_LITERATURE_SCOUT.to_string(),
        r#type: "agent".to_string(),
    };
    let mut proposal = new_proposal(
        "finding.add",
        StateTarget {
            r#type: "finding".to_string(),
            id: finding.id.clone(),
        },
        AGENT_ACTOR_ID_LITERATURE_SCOUT,
        "agent",
        reason,
        payload,
        vec![source_label],
        flags.to_vec(),
    );
    proposal.agent_run = Some(run.clone());
    proposal
}

/// Helper: discover PDF files in a folder, skipping hidden entries
/// and non-PDF files. Pulled out so the CLI can preview discovery
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_pdfs_filters_correctly() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.pdf"), b"%PDF-1.4").unwrap();
        std::fs::write(dir.path().join("b.txt"), b"not a pdf").unwrap();
        std::fs::write(dir.path().join(".hidden.pdf"), b"%PDF-1.4").unwrap();
        std::fs::write(dir.path().join("c.pdf"), b"%PDF-1.4").unwrap();

        let pdfs = discover_pdfs(dir.path()).unwrap();
        assert_eq!(pdfs.len(), 2);
        let names: Vec<String> = pdfs
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"a.pdf".to_string()));
        assert!(names.contains(&"c.pdf".to_string()));
        assert!(!names.contains(&".hidden.pdf".to_string()));
    }
}
