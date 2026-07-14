use crate::cli::print_state_report;
use crate::cli::{fail, fail_return, print_json};
use crate::cli_commands::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use vela_protocol::cli_style as style;
use vela_protocol::proposals;
use vela_protocol::repo;

pub(crate) fn cmd_proposals(action: ProposalAction) {
    match action {
        ProposalAction::List {
            frontier,
            status,
            json,
        } => {
            let frontier_state =
                repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let proposals_list = proposals::list(&frontier_state, status.as_deref());
            let payload = json!({
                "ok": true,
                "command": "proposals.list",
                "frontier": frontier_state.project.name,
                "status_filter": status,
                "summary": proposals::summary(&frontier_state),
                "proposals": proposals_list,
            });
            if json {
                print_json(&payload);
            } else {
                println!("vela proposals list");
                println!("  frontier: {}", frontier_state.project.name);
                println!(
                    "  proposals: {}",
                    payload["proposals"].as_array().map_or(0, Vec::len)
                );
            }
        }
        ProposalAction::Show {
            frontier,
            proposal_id,
            json,
        } => {
            let frontier_state =
                repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let proposal =
                proposals::show(&frontier_state, &proposal_id).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "proposals.show",
                "frontier": frontier_state.project.name,
                "proposal": proposal,
            });
            if json {
                print_json(&payload);
            } else {
                println!("vela proposals show");
                println!("  frontier: {}", frontier_state.project.name);
                println!("  proposal: {}", proposal_id);
                println!("  kind: {}", proposal.kind);
                println!("  status: {}", proposal.status);
            }
        }
        ProposalAction::Preview {
            frontier,
            proposal_id,
            reviewer: _,
            json,
        } => {
            let review = crate::review_material::ReviewProjection::one(&frontier, &proposal_id)
                .unwrap_or_else(|error| fail_return(&error.to_string()));
            let payload = json!({
                "ok": true,
                "command": "proposals.preview",
                "frontier": frontier.display().to_string(),
                "review": review,
            });
            if json {
                print_json(&payload);
            } else {
                println!("vela proposals preview");
                println!("  proposal: {}", proposal_id);
                println!("  claim: {}", review.brief.change.claim);
                for line in crate::cli::sign_session::render_decision_brief_lines(&review.brief) {
                    println!("    {line}");
                }
            }
        }
        ProposalAction::Import {
            frontier,
            source,
            json,
        } => {
            let report =
                proposals::import_from_path(&frontier, &source).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "proposals.import",
                "frontier": frontier.display().to_string(),
                "source": source.display().to_string(),
                "summary": {
                    "imported": report.imported,
                    "applied": report.applied,
                    "rejected": report.rejected,
                    "duplicates": report.duplicates,
                },
            });
            if json {
                print_json(&payload);
            } else {
                println!(
                    "Imported {} proposals into {}",
                    report.imported, report.wrote_to
                );
            }
        }
        ProposalAction::Validate { source, json } => {
            let report = proposals::validate_source(&source).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": report.ok,
                "command": "proposals.validate",
                "source": source.display().to_string(),
                "summary": {
                    "checked": report.checked,
                    "valid": report.valid,
                    "invalid": report.invalid,
                },
                "proposal_ids": report.proposal_ids,
                "errors": report.errors,
            });
            if json {
                print_json(&payload);
            } else if report.ok {
                println!("{} validated {} proposals", style::ok("ok"), report.valid);
            } else {
                println!(
                    "{} validated {} proposals, {} invalid",
                    style::lost("lost"),
                    report.valid,
                    report.invalid
                );
                for error in &report.errors {
                    println!("  · {error}");
                }
                std::process::exit(1);
            }
        }
        ProposalAction::Export {
            frontier,
            output,
            status,
            json,
        } => {
            let count = proposals::export_to_path(&frontier, &output, status.as_deref())
                .unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "proposals.export",
                "frontier": frontier.display().to_string(),
                "output": output.display().to_string(),
                "status": status,
                "exported": count,
            });
            if json {
                print_json(&payload);
            } else {
                println!("sealed · {count} proposals · {}", output.display());
            }
        }
        ProposalAction::Accept {
            frontier,
            proposal_id,
            reviewer,
            reason,
            key,
            json,
        } => {
            let reviewer = crate::cli_identity::resolve_decision_actor(reviewer.as_deref());
            let signing_key = crate::cli_identity::resolve_signing_key_opt(key.as_deref());
            let publish_opts = crate::config::git_publish::PublishOptions::new(false, false);
            let publication_preflight =
                crate::config::git_publish::publication_preflight(&frontier, &publish_opts);
            if let Err(outcome) = &publication_preflight
                && crate::config::git_publish::publication_is_busy(outcome)
            {
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Domain,
                    "another Vela write/publication owns this repository; the proposal was not accepted",
                    Some("retry after the active operation completes"),
                );
            }
            let event_id = proposals::accept_at_path_signed(
                &frontier,
                &proposal_id,
                &reviewer,
                &reason,
                signing_key.as_ref(),
            )
            .unwrap_or_else(|e| fail_return(&e));
            let publication = match publication_preflight {
                Ok(preflight) => {
                    let publish_opts = publish_opts.with_preflight(preflight);
                    crate::config::git_publish::publish_decision(
                        &frontier,
                        &format!("accept: {proposal_id}"),
                        std::slice::from_ref(&event_id),
                        &publish_opts,
                    )
                }
                Err(outcome) => outcome,
            };
            let operation_id =
                crate::operation_journal::operation_id("proposal-accept", event_id.as_bytes());
            let payload = json!({
                "ok": true,
                "command": "proposals.accept",
                "operation_id": operation_id,
                "frontier": frontier.display().to_string(),
                "proposal_id": proposal_id,
                "reviewer": reviewer,
                "applied_event_id": event_id,
                "publication": publication,
            });
            if json {
                print_json(&payload);
            } else {
                println!(
                    "{} accepted and applied proposal {}",
                    style::ok("ok"),
                    crate::cli::safe_text::inline(&proposal_id)
                );
                println!("  event: {}", crate::cli::safe_text::inline(&event_id));
                println!(
                    "  publication: {}",
                    crate::cli::safe_text::inline(
                        &serde_json::to_string(&publication)
                            .unwrap_or_else(|_| "unknown".to_string())
                    )
                );
                println!(
                    "  retained: {}",
                    crate::cli::safe_text::inline(&operation_id)
                );
                println!(
                    "  next: {}",
                    crate::cli::safe_text::inline(
                        publication
                            .recovery_command
                            .as_deref()
                            .unwrap_or("vela status --json")
                    )
                );
            }
        }
        ProposalAction::Reject {
            frontier,
            no_commit,
            no_push,
            proposal_id,
            reviewer,
            reason,
            key,
            json,
        } => {
            let reviewer = crate::cli_identity::resolve_decision_actor(reviewer.as_deref());
            let signing_key = crate::cli_identity::resolve_signing_key_opt(key.as_deref());
            let publish_opts = crate::config::git_publish::PublishOptions::new(no_commit, no_push);
            let publication_preflight =
                crate::config::git_publish::publication_preflight(&frontier, &publish_opts);
            if let Err(outcome) = &publication_preflight
                && crate::config::git_publish::publication_is_busy(outcome)
            {
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Domain,
                    "another Vela write/publication owns this repository; the proposal was not rejected",
                    Some("retry after the active operation completes"),
                );
            }
            proposals::reject_at_path_signed(
                &frontier,
                &proposal_id,
                &reviewer,
                &reason,
                signing_key.as_ref(),
            )
            .unwrap_or_else(|e| fail_return(&e));
            let publication = match publication_preflight {
                Ok(preflight) => {
                    let publish_opts = publish_opts.with_preflight(preflight);
                    crate::config::git_publish::publish_decision(
                        &frontier,
                        &format!("reject: {proposal_id}"),
                        &[],
                        &publish_opts,
                    )
                }
                Err(outcome) => outcome,
            };
            let operation_id =
                crate::operation_journal::operation_id("proposal-reject", proposal_id.as_bytes());
            let payload = json!({
                "ok": true,
                "command": "proposals.reject",
                "operation_id": operation_id,
                "frontier": frontier.display().to_string(),
                "proposal_id": proposal_id,
                "reviewer": reviewer,
                "status": "rejected",
                "signed": signing_key.is_some(),
                "publication": publication,
            });
            if json {
                print_json(&payload);
            } else {
                println!(
                    "{} rejected proposal {}{}",
                    style::warn("rejected"),
                    crate::cli::safe_text::inline(&proposal_id),
                    if signing_key.is_some() {
                        " (signed review.rejected event)"
                    } else {
                        ""
                    }
                );
                println!(
                    "  publication: {}",
                    crate::cli::safe_text::inline(
                        &serde_json::to_string(&publication)
                            .unwrap_or_else(|_| "unknown".to_string())
                    )
                );
                println!(
                    "  retained: {}",
                    crate::cli::safe_text::inline(&operation_id)
                );
                println!(
                    "  next: {}",
                    crate::cli::safe_text::inline(
                        publication
                            .recovery_command
                            .as_deref()
                            .unwrap_or("vela status --json")
                    )
                );
            }
        }
    }
}

// ── Finding-verb handlers (shared by the top-level alias + `vela finding`) ──
// Extracted so `vela note …` (hidden top-level) and `vela finding note …`
// (canonical) dispatch to one body.

pub(crate) fn cmd_finding_note(
    frontier: PathBuf,
    finding_id: String,
    text: String,
    author: String,
    apply: bool,
    json: bool,
) {
    let report = vela_protocol::state::add_note(&frontier, &finding_id, &text, &author, apply)
        .unwrap_or_else(|e| fail_return(&e));
    print_state_report(&report, json);
}

pub(crate) fn cmd_finding_caveat(
    frontier: PathBuf,
    finding_id: String,
    text: String,
    author: String,
    apply: bool,
    json: bool,
) {
    let report =
        vela_protocol::state::caveat_finding(&frontier, &finding_id, &text, &author, apply)
            .unwrap_or_else(|e| fail_return(&e));
    print_state_report(&report, json);
}

pub(crate) fn cmd_finding_revise(
    frontier: PathBuf,
    finding_id: String,
    confidence: f64,
    reason: String,
    reviewer: String,
    apply: bool,
    json: bool,
) {
    let report = vela_protocol::state::revise_confidence(
        &frontier,
        &finding_id,
        vela_protocol::state::ReviseOptions {
            confidence,
            reason,
            reviewer,
        },
        apply,
    )
    .unwrap_or_else(|e| fail_return(&e));
    print_state_report(&report, json);
}

pub(crate) fn cmd_finding_reject(
    frontier: PathBuf,
    finding_id: String,
    reason: String,
    reviewer: String,
    apply: bool,
    json: bool,
) {
    let report =
        vela_protocol::state::reject_finding(&frontier, &finding_id, &reviewer, &reason, apply)
            .unwrap_or_else(|e| fail_return(&e));
    print_state_report(&report, json);
}

/// Record a review verdict on a finding. An accept (the default status) emits a
/// human-keyed `finding.reviewed` event, setting `review_state = Accepted` so
/// the derived frontier state becomes `Established`. This is the porcelain over
/// `state::review_finding`; the finding must already exist (assert it first with
/// `land`). The custody line is unchanged: only a human key applies it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_finding_review(
    frontier: PathBuf,
    finding_id: String,
    status: String,
    reason: String,
    confidence: Option<f64>,
    reviewer: String,
    apply: bool,
    json: bool,
) {
    let report = vela_protocol::state::review_finding(
        &frontier,
        &finding_id,
        vela_protocol::state::ReviewOptions {
            status,
            reason,
            reviewer,
            confidence,
        },
        apply,
    )
    .unwrap_or_else(|e| fail_return(&e));
    print_state_report(&report, json);
}

/// Record a claim-granularity attribution on a finding. Parses the string
/// enum args and calls `state::record_contribution` (descriptive provenance,
/// agent-draftable; `vouched` must be a human).
#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_finding_contribution(
    frontier: PathBuf,
    finding_id: String,
    unit: String,
    unit_type: String,
    agent_kind: String,
    agent_id: String,
    model: Option<String>,
    model_version: Option<String>,
    role: String,
    basis: String,
    actor: String,
    apply: bool,
    json_out: bool,
) {
    use vela_protocol::bundle::{AgentKind, Contribution, ContributionRole, ContributionUnitType};
    let unit_type: ContributionUnitType =
        serde_json::from_value(json!(unit_type)).unwrap_or_else(|_| {
            fail_return("invalid --unit-type (evidence_span | lean_decl | step | whole)")
        });
    let agent_kind: AgentKind = serde_json::from_value(json!(agent_kind))
        .unwrap_or_else(|_| fail_return("invalid --agent-kind (human | agent | model)"));
    let role: ContributionRole = serde_json::from_value(json!(role)).unwrap_or_else(|_| {
        fail_return(
            "invalid --role (originated | derived | formalized | extracted | reviewed | vouched)",
        )
    });
    let contribution = Contribution {
        unit,
        unit_type,
        agent_kind,
        agent_id,
        model,
        model_version,
        role,
        basis,
    };
    let report = vela_protocol::state::record_contribution(
        &frontier,
        &finding_id,
        contribution,
        &actor,
        apply,
    )
    .unwrap_or_else(|e| fail_return(&e));
    print_state_report(&report, json_out);
}

/// The derived credit view for a finding (read-only projection). Renders the
/// accountable human author(s) of record, the disclosed contributors, and the
/// originating agents. A machine never appears as an author.
pub(crate) fn cmd_credit(frontier: &Path, finding_id: &str, json_out: bool) {
    let source = repo::detect(frontier).unwrap_or_else(|e| fail_return(&e));
    let proj = repo::load(&source).unwrap_or_else(|e| fail_return(&e));
    let view = vela_protocol::credit::credit(&proj, finding_id)
        .unwrap_or_else(|| fail_return(&format!("no such finding: {finding_id}")));
    if json_out {
        print_json(&json!({
            "command": "credit",
            "schema": "vela.credit.v0.1",
            "credit": view,
        }));
        return;
    }
    println!("credit · {finding_id}");
    if view.author_of_record.is_empty() {
        println!("  author of record: (none — no accountable author yet)");
    } else {
        println!("  author of record: {}", view.author_of_record.join(", "));
    }
    if view.contributors.is_empty() {
        println!("  contributors:     (none recorded)");
    } else {
        println!("  contributors:");
        for c in &view.contributors {
            println!(
                "    {} [{}] {} — {}",
                c.agent_id, c.agent_kind, c.role, c.unit
            );
        }
    }
    if !view.originating_agents.is_empty() {
        println!("  originating agents (disclosed, not authors):");
        for c in &view.originating_agents {
            println!(
                "    {} [{}] originated {}",
                c.agent_id, c.agent_kind, c.unit
            );
        }
    }
    println!("  {}", view.statement);
}

pub(crate) fn cmd_finding_retract(
    source: PathBuf,
    finding_id: String,
    reason: String,
    reviewer: String,
    apply: bool,
    json: bool,
) {
    let report =
        vela_protocol::state::retract_finding(&source, &finding_id, &reviewer, &reason, apply)
            .unwrap_or_else(|e| fail_return(&e));
    print_state_report(&report, json);
}

pub(crate) fn cmd_artifact_retract(
    frontier: PathBuf,
    artifact_id: String,
    reason: String,
    actor: String,
    json: bool,
) {
    let project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
    if !project
        .artifacts
        .iter()
        .any(|artifact| artifact.id == artifact_id)
    {
        crate::cli::fail_not_found::<()>(
            &format!("no artifact '{artifact_id}' in this frontier"),
            "inspect the frontier with `vela status <frontier> --json`",
        );
    }
    let report = vela_protocol::state::retract_artifact(&frontier, &artifact_id, &actor, &reason)
        .unwrap_or_else(|e| fail_return(&e));
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .expect("failed to serialize artifact lifecycle report")
        );
    } else {
        println!("Artifact retirement proposal recorded");
        println!("  frontier: {}", report.frontier);
        println!("  artifact: {}", report.artifact_id);
        println!("  proposal: {}", report.proposal_id);
        println!("  status:   {}", report.status);
        println!("  route:    {}", report.route);
    }
}

/// A signed statement-faithfulness attestation (`vsa_`) — the human
/// judgment that a FORMAL statement faithfully encodes an INFORMAL problem.
/// Reserved for `reviewer:` actors by design: `StatementAttestation::build`
/// refuses any agent, so a model can LAND a finding but never attest that a
/// formalization means what a human meant. Mirrors `cmd_claim`'s
/// load -> event -> apply -> sign -> save path; the reducer
/// (`apply_statement_attested`) re-verifies the attestation signature.
/// One faithfulness verdict applied into an already-loaded project: build the
/// `vsa_`, emit and sign the `statement.attested` event under the reviewer's
/// key, push it. Does NOT save, so the single and `--batch` paths share it and
/// the batch path signs N verdicts under one key read and one save. Returns the
/// attestation id, or a human-readable error (never exits).
#[allow(clippy::too_many_arguments)]
fn apply_one_faithfulness(
    project: &mut vela_protocol::project::Project,
    target: &str,
    verdict: &str,
    informal_ref: String,
    formal_ref: String,
    formal_statement_hash: String,
    note: String,
    by: &str,
    signing_key: &ed25519_dalek::SigningKey,
) -> Result<String, String> {
    use vela_protocol::statement_attestation::{
        AttestationDraft, FaithfulnessVerdict, StatementAttestation,
    };
    let verdict_enum = match verdict.to_ascii_lowercase().as_str() {
        "faithful" => FaithfulnessVerdict::Faithful,
        "variant" => FaithfulnessVerdict::Variant,
        "unfaithful" => FaithfulnessVerdict::Unfaithful,
        other => {
            return Err(format!(
                "--verdict must be faithful|variant|unfaithful, got '{other}'"
            ));
        }
    };
    if !project.findings.iter().any(|f| f.id == target) {
        return Err(format!("target finding {target} not found in frontier"));
    }
    let att = StatementAttestation::build(
        AttestationDraft {
            target: target.to_string(),
            informal_ref,
            formal_ref,
            formal_statement_hash,
            verdict: verdict_enum,
            note,
            attested_by: by.to_string(),
            attested_at: chrono::Utc::now().to_rfc3339(),
        },
        signing_key,
    )?;
    let attestation_id = att.id.clone();
    let mut event =
        vela_protocol::events::new_finding_event(vela_protocol::events::FindingEventInput {
            kind: "statement.attested",
            finding_id: target,
            actor_id: by,
            actor_type: vela_protocol::events::actor_kind(by),
            reason: "statement faithfulness attestation",
            before_hash: "sha256:null",
            after_hash: "sha256:null",
            payload: serde_json::json!({ "attestation": att }),
            caveats: Vec::new(),
            timestamp: None,
        });
    vela_protocol::reducer::apply_event(project, &event)?;
    event.signature = Some(vela_protocol::sign::sign_event(&event, signing_key)?);
    project.events.push(event);
    Ok(attestation_id)
}

/// Guard shared by both faithfulness paths: statement faithfulness is human
/// judgment, so the attester must be a `reviewer:` actor and a human key must
/// be present. `StatementAttestation::build` refuses any agent, but failing
/// early here gives a clearer message than a build error.
fn resolve_faithfulness_signer(
    reviewer: Option<String>,
    key: Option<&Path>,
) -> (String, ed25519_dalek::SigningKey) {
    let by = crate::cli_identity::resolve_actor(reviewer.as_deref());
    if !by.starts_with("reviewer:") {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Custody,
            &format!(
                "attest: statement faithfulness is human judgment by design; reviewer must be a reviewer: actor, got '{by}'"
            ),
            Some("run under a human identity: `vela id show`, or pass --as reviewer:<handle>"),
        );
    }
    let signing_key = crate::cli_identity::resolve_signing_key(key);
    (by, signing_key)
}

/// `vela sign --batch <file>`: sign a whole list of fidelity verdicts
/// under ONE key read and ONE save, instead of one keyed command per
/// verdict. Each verdict is still a human judgment signed by the reviewer's own
/// key; batching only removes the per-verdict repetition (the migration of the
/// overrides table is the motivating case). The file is JSON, either a bare
/// array or `{ "verdicts": [ ... ] }`, each row:
/// `{ target, verdict, informal_ref, formal_ref, formal_statement_hash, note }`.
/// All-or-nothing: if any row fails to build, nothing is saved.
pub(crate) fn cmd_review_fidelity_batch(
    frontier: PathBuf,
    batch_path: PathBuf,
    reviewer: Option<String>,
    key: Option<PathBuf>,
    json: bool,
) {
    #[derive(serde::Deserialize)]
    struct VerdictRow {
        target: String,
        verdict: String,
        #[serde(default)]
        informal_ref: String,
        #[serde(default)]
        formal_ref: String,
        #[serde(default)]
        formal_statement_hash: String,
        #[serde(default)]
        note: String,
    }
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Batch {
        Wrapped { verdicts: Vec<VerdictRow> },
        Bare(Vec<VerdictRow>),
    }
    let raw = std::fs::read_to_string(&batch_path)
        .unwrap_or_else(|e| fail_return(&format!("attest: read {}: {e}", batch_path.display())));
    let rows = match serde_json::from_str::<Batch>(&raw)
        .unwrap_or_else(|e| fail_return(&format!("attest: parse {}: {e}", batch_path.display())))
    {
        Batch::Wrapped { verdicts } => verdicts,
        Batch::Bare(v) => v,
    };
    if rows.is_empty() {
        fail(&format!("attest: {} has no verdicts", batch_path.display()));
    }
    let (by, signing_key) = resolve_faithfulness_signer(reviewer, key.as_deref());
    let mut project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
    let mut applied = Vec::with_capacity(rows.len());
    for (i, row) in rows.into_iter().enumerate() {
        let target = row.target.clone();
        let verdict = row.verdict.clone();
        let id = apply_one_faithfulness(
            &mut project,
            &row.target,
            &row.verdict,
            row.informal_ref,
            row.formal_ref,
            row.formal_statement_hash,
            row.note,
            &by,
            &signing_key,
        )
        .unwrap_or_else(|e| fail_return(&format!("attest: verdict {i} ({target}): {e}")));
        applied.push(json!({ "attestation_id": id, "target": target, "verdict": verdict }));
    }
    repo::save_to_path(&frontier, &project).unwrap_or_else(|e| fail_return(&e));
    if json {
        print_json(&json!({
            "ok": true, "command": "attest.faithfulness.batch",
            "count": applied.len(), "by": by, "attestations": applied,
        }));
    } else {
        println!(
            "{} signed {} faithfulness verdict(s) by {by} in one batch",
            style::ok("ok"),
            applied.len(),
        );
        for a in &applied {
            println!(
                "  {} {} ({})",
                a["attestation_id"].as_str().unwrap_or(""),
                a["target"].as_str().unwrap_or(""),
                a["verdict"].as_str().unwrap_or(""),
            );
        }
    }
}
