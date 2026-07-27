//! Current Claim projection over current or explicitly historical source bytes.

use crate::cli::{fail_return, print_json, wrap_line};

use std::path::Path;
use vela_protocol::cli_style as style;
use vela_protocol::repo;
use vela_protocol::state;

use colored::Colorize;
use serde_json::{Value, json};

pub(crate) fn cmd_claim_show(frontier: &Path, claim_id: &str, view: &str, json_out: bool) {
    crate::ui::set_mode("claim.show", json_out);
    if frontier.join(".vela/epoch.json").is_file() {
        let projection = crate::current_read::claim_payload(frontier, claim_id, view)
            .unwrap_or_else(|error| fail_return(&error));
        if json_out {
            print_json(&projection);
        } else {
            println!(
                "{}",
                serde_json::to_string_pretty(&projection)
                    .expect("serialize current Claim projection")
            );
        }
        return;
    }
    let project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));
    let ctx = state::finding_context(&project, claim_id).unwrap_or_else(|_| {
        crate::cli::fail_not_found(
            &format!("no Claim or historical Finding '{claim_id}' in this frontier"),
            "inspect recent history with `vela log .`, or use a full stable id",
        )
    });
    let finding = project
        .findings
        .iter()
        .find(|finding| finding.id == claim_id)
        .expect("finding_context succeeded for the same historical Finding");
    let attachments = project
        .verifier_attachments
        .iter()
        .filter(|attachment| attachment.target == claim_id)
        .cloned()
        .collect::<Vec<_>>();
    let gate = vela_protocol::verifier_attachment::derive_gate_status(
        &vela_protocol::verifier_attachment::claim_digest(&finding.assertion.text),
        &attachments,
    );
    let verification_records = attachments
        .iter()
        .map(|attachment| {
            json!({
                "source_era": "historical",
                "source_schema": vela_protocol::verifier_attachment::ATTACHMENT_SCHEMA,
                "historical_verifier_attachment_id": attachment.id,
                "record": attachment,
            })
        })
        .collect::<Vec<_>>();
    let projection = match view {
        "record" => json!({
            "ok": true,
            "command": "claim.show",
            "schema": "vela.claim-view.v1",
            "view": "record",
            "frontier_id": project.frontier_id(),
            "claim_id": claim_id,
            "source_era": "historical",
            "source_schema": vela_protocol::project::VELA_SCHEMA_URL,
            "historical_finding_id": claim_id,
            "record": ctx,
        }),
        "standing" => json!({
            "ok": true,
            "command": "claim.show",
            "schema": "vela.claim-view.v1",
            "view": "standing",
            "frontier_id": project.frontier_id(),
            "claim_id": claim_id,
            "source_era": "historical",
            "source_schema": vela_protocol::project::VELA_SCHEMA_URL,
            "historical_finding_id": claim_id,
            "review_state": finding.flags.review_state,
            "retracted": finding.flags.retracted,
            "contested": finding.flags.contested,
            "superseded": finding.flags.superseded,
            "trust_tier": ctx.get("trust_tier"),
            "verification": {
                "status": gate.status,
                "reasons": gate.reasons,
            },
            "condition_records": ctx.get("condition_records"),
        }),
        "evidence" => json!({
            "ok": true,
            "command": "claim.show",
            "schema": "vela.claim-view.v1",
            "view": "evidence",
            "frontier_id": project.frontier_id(),
            "claim_id": claim_id,
            "source_era": "historical",
            "source_schema": vela_protocol::project::VELA_SCHEMA_URL,
            "historical_finding_id": claim_id,
            "evidence_atoms": ctx.get("evidence_atoms"),
            "verification_records": verification_records,
            "artifacts": project.artifacts.iter()
                .filter(|artifact| artifact.target_findings.iter().any(|target| target == claim_id))
                .collect::<Vec<_>>(),
        }),
        "attribution" => json!({
            "ok": true,
            "command": "claim.show",
            "schema": "vela.claim-view.v1",
            "view": "attribution",
            "frontier_id": project.frontier_id(),
            "claim_id": claim_id,
            "source_era": "historical",
            "source_schema": vela_protocol::project::VELA_SCHEMA_URL,
            "historical_finding_id": claim_id,
            "attribution": vela_protocol::credit::credit(&project, claim_id),
        }),
        other => crate::ui::fail_with(
            crate::ui::ErrorKind::Usage,
            &format!("unsupported Claim view {other:?}"),
            Some("use --view record, standing, evidence, or attribution"),
        ),
    };
    if json_out {
        print_json(&projection);
        return;
    }
    if view != "record" {
        println!("claim · {claim_id} · {view} · historical Finding era");
        println!(
            "{}",
            serde_json::to_string_pretty(&projection).expect("serialize finding projection")
        );
        return;
    }
    let finding = ctx.get("finding").cloned().unwrap_or(Value::Null);
    println!();
    println!(
        "  {}",
        format!("VELA · CLAIM · {claim_id} · HISTORICAL FINDING ERA")
            .to_uppercase()
            .dimmed()
    );
    println!("  {}", style::tick_row(60));
    println!(
        "  assertion: {}",
        wrap_line(
            finding
                .pointer("/assertion/text")
                .and_then(Value::as_str)
                .unwrap_or(""),
            82
        )
    );
    let cs = ctx
        .get("confidence_score")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let cb = ctx
        .get("confidence_basis")
        .and_then(Value::as_str)
        .unwrap_or("unspecified");
    let rv = ctx
        .get("reviewed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let rk = ctx
        .get("reviewed_by_kind")
        .and_then(Value::as_str)
        .unwrap_or("none");
    println!("  confidence: {cs:.3}  (basis: {cb}) [reviewed: {rv} by {rk}]");
    if cs >= 0.7 && !rv {
        println!("  note: confidence >=0.70 on an unreviewed basis — not adjudicated evidence");
    }
    // Phase 1A: the verification trust tier, rendered DISTINCT from human accept.
    let tier = ctx
        .get("trust_tier")
        .and_then(Value::as_str)
        .unwrap_or("candidate");
    let tier_line = match tier {
        "accepted" => "trust tier: accepted (human, key-custody)".green(),
        "machine_verified" => {
            "trust tier: machine_verified (deterministic exact-lane; not human-accepted)".cyan()
        }
        "schema_checked" => "trust tier: schema_checked".yellow(),
        _ => "trust tier: candidate".dimmed(),
    };
    println!("  {tier_line}");
    // The verification gate (G1–G4), derived — never stored. Reviewer
    // accept and machine seal are DIFFERENT facts; a finding can be
    // human-accepted and still needs_verification, and hiding that gap
    // is the exact failure the gate exists to prevent.
    {
        use vela_protocol::verifier_attachment::{GateStatus, claim_digest, derive_gate_status};
        if let Some(bundle) = project.findings.iter().find(|b| b.id == claim_id) {
            let attachments: Vec<_> = project
                .verifier_attachments
                .iter()
                .filter(|a| a.target == claim_id)
                .cloned()
                .collect();
            let outcome = derive_gate_status(&claim_digest(&bundle.assertion.text), &attachments);
            let status_json = serde_json::json!(outcome.status);
            let status_str = status_json.as_str().unwrap_or("unknown");
            let line = format!(
                "verification: {status_str} ({} historical record{})",
                attachments.len(),
                if attachments.len() == 1 { "" } else { "s" }
            );
            let line = match outcome.status {
                GateStatus::Verified => line.green(),
                GateStatus::Refuted => line.red(),
                _ => line.yellow(),
            };
            println!("  {line}");
            for reason in outcome.reasons.iter().take(3) {
                println!("    · {reason}");
            }
        }
    }
    if let Some(atoms) = ctx.get("evidence_atoms").and_then(Value::as_array) {
        println!("  evidence atoms: {}", atoms.len());
        for a in atoms.iter().take(12) {
            let claim: String = a
                .get("measurement_or_claim")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(100)
                .collect();
            println!(
                "    - [{}] {} :: {}",
                a.get("source_id").and_then(Value::as_str).unwrap_or(""),
                a.get("locator")
                    .and_then(Value::as_str)
                    .unwrap_or("(no locator)"),
                claim
            );
        }
    }
    if let Some(cr) = ctx.get("condition_records").and_then(Value::as_array)
        && !cr.is_empty()
    {
        println!("  condition records: {}", cr.len());
    }
    if let Some(links) = finding.get("links").and_then(Value::as_array)
        && !links.is_empty()
    {
        println!("  links:");
        for l in links.iter().take(12) {
            println!(
                "    - {} -> {} ({})",
                l.get("type").and_then(Value::as_str).unwrap_or(""),
                l.get("target").and_then(Value::as_str).unwrap_or(""),
                l.get("inferred_by").and_then(Value::as_str).unwrap_or("")
            );
        }
    }
    // Attribution: the derived credit view — disclosed producers (machines
    // included) and the accountable author (a valid human signature, or "none
    // yet"). Shown only when the finding carries attribution data, so plain
    // findings stay uncluttered. The `vela credit` projection is the full view.
    if let Some(view) = vela_protocol::credit::credit(&project, claim_id)
        && !view.contributors.is_empty()
    {
        println!("  contributions:");
        for c in view.contributors.iter().take(12) {
            println!(
                "    - {} [{}] {} — {}",
                c.agent_id, c.agent_kind, c.role, c.unit
            );
        }
        let author = if view.author_of_record.is_empty() {
            "no accountable author yet".to_string()
        } else {
            view.author_of_record.join(", ")
        };
        println!(
            "    credit: {author}  (full view: `vela claim show . {claim_id} --view attribution`)"
        );
    }
    println!(
        "  canonical events: {}",
        ctx.get("events")
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    );
}
