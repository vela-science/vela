//! Target-index sealing and read-only diagnosis porcelain.
//!
//! Domain tools own candidate target semantics. Vela only derives and checks
//! the exact Git, repository-state, and packet bindings needed to make those
//! semantics a safe producer offer. None of these commands grants scientific
//! authority.

use std::path::Path;

use serde::Serialize;
use vela_edge::frontier_repository::RepositoryTrustAnchor;
use vela_edge::repository_write::{
    RepositoryTrustAnchorV1, load_repository_trust_anchor_from_home,
    verify_repository_for_write_with_authority_events,
};
use vela_edge::target_index::{
    TargetIndexInspectionSummary, TargetIndexRepairReport, TargetIndexSealPlan,
    TargetIndexTargetInspection, inspect_target_index_target_with_trust_anchor_and_authority,
    prepare_target_index_seal_install, prepare_target_index_seal_with_authority_events,
    target_index_inspection_summary_with_trust_anchor_and_authority,
    target_index_repair_report_with_trust_anchor_and_authority, validate_target_id,
};

use crate::cli_commands::TargetIndexAction;
use crate::ui::{self, ErrorKind};

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct SealOutput {
    schema: &'static str,
    ok: bool,
    command: &'static str,
    mode: &'static str,
    changed: bool,
    wrote: Vec<&'static str>,
    plan: TargetIndexSealPlan,
    next_command: String,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct RepairOutput {
    schema: &'static str,
    ok: bool,
    command: &'static str,
    report: TargetIndexRepairReport,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct InspectOutput {
    schema: &'static str,
    ok: bool,
    command: &'static str,
    frontier_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<TargetIndexInspectionSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<TargetIndexTargetInspection>,
    repair_command: String,
}

fn print_json<T: Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|error| ui::fail_with(
            ErrorKind::Internal,
            &error.to_string(),
            None
        ))
    );
}

fn frontier_display(frontier: &Path) -> String {
    frontier.display().to_string()
}

fn load(frontier: &Path) -> vela_protocol::project::Project {
    vela_protocol::repo::load_from_path(frontier)
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None))
}

pub(crate) fn load_user_repository_trust_anchor(
    frontier_id: &str,
) -> Result<Option<vela_edge::repository_write::LoadedRepositoryTrustAnchorV1>, String> {
    // This is security-critical consumer trust, not ordinary configuration.
    // Resolve the operating-system account home exactly as the canonical write
    // gate does; `$HOME` is process input and must not redirect the pin lookup.
    let home =
        crate::frontier_txn::operating_system_account_home().map_err(|error| error.to_string())?;
    load_repository_trust_anchor_from_home(&home, frontier_id)
}

pub(crate) fn boundary_anchor(anchor: &RepositoryTrustAnchorV1) -> RepositoryTrustAnchor {
    RepositoryTrustAnchor {
        boundary_content_root: anchor.boundary_content_root.clone(),
        administrator_public_key: anchor.administrator_public_key.clone(),
    }
}

pub(crate) fn load_verified_authority_events(
    frontier: &Path,
    project: &vela_protocol::project::Project,
) -> Result<Vec<vela_protocol::authority::AuthorityEventV1>, String> {
    crate::cli::load_repository_authority(frontier, project)
        .map_err(|error| format!("verify repository-authority history: {error}"))
        .map(|authority| authority.map_or_else(Vec::new, |value| value.history.authority_events))
}

fn cmd_seal(frontier: &Path, candidate: &Path, apply: bool, json: bool) {
    ui::set_mode("target-index.seal", json);
    let project = load(frontier);
    let loaded_anchor = load_user_repository_trust_anchor(&project.frontier_id())
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
    let repository_anchor = loaded_anchor
        .as_ref()
        .map(|loaded| boundary_anchor(&loaded.anchor));
    let authority_events = load_verified_authority_events(frontier, &project)
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
    let plan = prepare_target_index_seal_with_authority_events(
        frontier,
        candidate,
        env!("CARGO_PKG_VERSION"),
        repository_anchor.as_ref(),
        &authority_events,
    )
    .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
    let changed = if apply {
        // Pin the repository root, destination parent, and exact targets.json
        // preimage before the write gate. The install revalidates those same
        // identities and bytes after the gate, closing the path-swap window.
        let prepared_install = prepare_target_index_seal_install(frontier, &plan)
            .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
        let write_project = load(frontier);
        verify_repository_for_write_with_authority_events(
            frontier,
            &write_project,
            loaded_anchor.as_ref().map(|loaded| &loaded.anchor),
            &authority_events,
        )
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error.to_string(), None));
        prepared_install
            .install()
            .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None))
    } else {
        false
    };
    let next_command = if apply {
        format!(
            "git -C {} add -- targets.json {}",
            frontier_display(frontier),
            plan.packet_paths.join(" ")
        )
    } else {
        format!(
            "vela target-index seal {} --candidate {} --apply --json",
            frontier_display(frontier),
            candidate.display()
        )
    };
    let output = SealOutput {
        schema: "vela.target-index-seal.v1",
        ok: true,
        command: "target-index.seal",
        mode: if apply { "apply" } else { "check" },
        changed,
        wrote: if changed {
            vec!["targets.json"]
        } else {
            Vec::new()
        },
        plan,
        next_command,
    };
    if json {
        print_json(&output);
    } else {
        ui::header(
            "TARGET INDEX",
            frontier.to_string_lossy().as_ref(),
            Some(output.mode),
        );
        println!("  root           {}", output.plan.index_root);
        println!("  candidate      {}", output.plan.candidate_path);
        println!("  packets        {}", output.plan.packet_paths.len());
        println!(
            "  wrote          {}",
            if output.wrote.is_empty() {
                "nothing"
            } else {
                "targets.json"
            }
        );
        println!("  next           {}", output.next_command);
    }
}

fn cmd_repair(frontier: &Path, json: bool) {
    ui::set_mode("target-index.repair", json);
    let project = load(frontier);
    let loaded_anchor = load_user_repository_trust_anchor(&project.frontier_id())
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
    let repository_anchor = loaded_anchor
        .as_ref()
        .map(|loaded| boundary_anchor(&loaded.anchor));
    let authority_events = load_verified_authority_events(frontier, &project)
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
    let report = target_index_repair_report_with_trust_anchor_and_authority(
        &project,
        frontier,
        &frontier_display(frontier),
        repository_anchor.as_ref(),
        &authority_events,
    )
    .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None))
    .unwrap_or_else(|| {
        ui::fail_with(
            ErrorKind::NotFound,
            "this Frontier has no targets.json",
            Some("generate a domain-owned target-index candidate first"),
        )
    });
    let output = RepairOutput {
        schema: "vela.target-index-repair-output.v1",
        ok: true,
        command: "target-index.repair",
        report,
    };
    if json {
        print_json(&output);
    } else {
        ui::header(
            "TARGET INDEX",
            frontier.to_string_lossy().as_ref(),
            Some("repair report; no writes"),
        );
        if output.report.codes.is_empty() {
            println!("  status         fresh");
        } else {
            println!("  stale codes    {}", output.report.codes.join(", "));
        }
        println!("  next           {}", output.report.repair_command);
    }
}

fn cmd_inspect(frontier: &Path, target_id: Option<&str>, json: bool) {
    ui::set_mode("target-index.inspect", json);
    let project = load(frontier);
    let loaded_anchor = load_user_repository_trust_anchor(&project.frontier_id())
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
    let repository_anchor = loaded_anchor
        .as_ref()
        .map(|loaded| boundary_anchor(&loaded.anchor));
    let repair_command = format!(
        "vela target-index repair {} --json",
        frontier_display(frontier)
    );
    let (summary, target) = if let Some(target_id) = target_id {
        validate_target_id(target_id)
            .unwrap_or_else(|error| ui::fail_with(ErrorKind::Usage, &error, None));
        let authority_events = load_verified_authority_events(frontier, &project)
            .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
        let target = inspect_target_index_target_with_trust_anchor_and_authority(
            &project,
            frontier,
            target_id,
            repository_anchor.as_ref(),
            &authority_events,
        )
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None))
        .unwrap_or_else(|| {
            ui::fail_with(
                ErrorKind::NotFound,
                &format!("exact target ID {target_id:?} is absent from targets.json"),
                Some(&repair_command),
            )
        });
        (None, Some(target))
    } else {
        let authority_events = load_verified_authority_events(frontier, &project)
            .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
        let summary = target_index_inspection_summary_with_trust_anchor_and_authority(
            &project,
            frontier,
            &frontier_display(frontier),
            repository_anchor.as_ref(),
            &authority_events,
        )
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None))
        .unwrap_or_else(|| {
            ui::fail_with(
                ErrorKind::NotFound,
                "this Frontier has no targets.json",
                Some("generate and seal a domain-owned target-index candidate"),
            )
        });
        (Some(summary), None)
    };
    let output = InspectOutput {
        schema: "vela.target-index-inspect.v1",
        ok: true,
        command: "target-index.inspect",
        frontier_id: project.frontier_id(),
        summary,
        target,
        repair_command,
    };
    if json {
        print_json(&output);
    } else if let Some(target) = &output.target {
        ui::header("TARGET", &target.target_id, Some(&target.state));
        println!("  title          {}", target.title);
        println!("  index          {}", target.index_root);
        println!("  historical     {}", target.historical_only);
        println!("  actionable     false (inspection only)");
        if !target.codes.is_empty() {
            println!("  codes          {}", target.codes.join(", "));
        }
        println!("  repair         {}", output.repair_command);
    } else if let Some(summary) = &output.summary {
        ui::header(
            "TARGET INDEX",
            frontier.to_string_lossy().as_ref(),
            Some(&summary.index_schema),
        );
        println!("  root           {}", summary.index_root);
        println!("  open           {}", summary.configured_open);
        println!("  stale          {}", summary.stale_open);
        println!("  historical     {}", summary.historical_only);
        println!("  repair         {}", summary.repair_command);
    }
}

pub(crate) fn cmd_target_index(action: TargetIndexAction) {
    match action {
        TargetIndexAction::Seal {
            frontier,
            candidate,
            check: _,
            apply,
            json,
        } => cmd_seal(&frontier, &candidate, apply, json),
        TargetIndexAction::Repair { frontier, json } => cmd_repair(&frontier, json),
        TargetIndexAction::Inspect {
            frontier,
            target_id,
            json,
        } => cmd_inspect(&frontier, target_id.as_deref(), json),
    }
}
