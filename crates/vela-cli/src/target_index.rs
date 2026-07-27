//! Target-index sealing and read-only diagnosis porcelain.
//!
//! Domain tools own candidate target semantics. Vela only derives and checks
//! the exact Git, repository-state, and packet bindings needed to make those
//! semantics a safe producer offer. None of these commands grants scientific
//! authority.

use std::path::Path;

use serde::Serialize;
use vela_edge::target_index::{
    CurrentTargetIndexAssessment, TargetIndexInspectionSummary, TargetIndexRepairReport,
    TargetIndexSealPlan, TargetIndexTargetInspection, assess_current_target_index,
    prepare_current_target_index_seal, prepare_current_target_index_seal_install,
    validate_target_id,
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

fn load_current(frontier: &Path) -> vela_protocol::current_repository::CurrentRepositoryV2 {
    crate::current_repository::verify_current_repository_allow_derived_drift_at(frontier)
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None))
}

fn assess_current(
    frontier: &Path,
    repository: &vela_protocol::current_repository::CurrentRepositoryV2,
) -> CurrentTargetIndexAssessment {
    let repository_root = repository
        .canonical_root()
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
    assess_current_target_index(
        frontier,
        &repository.frontier_id,
        &repository.epoch_id,
        &repository_root,
    )
    .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None))
    .unwrap_or_else(|| {
        ui::fail_with(
            ErrorKind::NotFound,
            "this Frontier has no targets.json",
            Some("generate and seal a domain-owned target-index candidate"),
        )
    })
}

fn assessment_codes(
    assessment: &CurrentTargetIndexAssessment,
    target_id: Option<&str>,
) -> Vec<&'static str> {
    let mut codes = assessment
        .global_issues
        .iter()
        .map(|issue| issue.code)
        .chain(
            target_id
                .and_then(|target_id| assessment.target_issues.get(target_id))
                .into_iter()
                .flatten()
                .map(|issue| issue.code),
        )
        .collect::<Vec<_>>();
    codes.sort_unstable();
    codes.dedup();
    codes
}

fn cmd_seal(frontier: &Path, candidate: &Path, apply: bool, json: bool) {
    ui::set_mode("target-index.seal", json);
    let repository = load_current(frontier);
    let repository_root = repository
        .canonical_root()
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
    let plan = prepare_current_target_index_seal(
        frontier,
        candidate,
        env!("CARGO_PKG_VERSION"),
        &repository.frontier_id,
        &repository.epoch_id,
        &repository_root,
    )
    .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
    let changed = if apply {
        // Pin the repository root, destination parent, and exact targets.json
        // preimage before the write gate. The install revalidates those same
        // identities and bytes after the gate, closing the path-swap window.
        let prepared_install = prepare_current_target_index_seal_install(frontier, &plan)
            .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
        let write_repository = load_current(frontier);
        if write_repository
            .canonical_root()
            .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None))
            != repository_root
        {
            ui::fail_with(
                ErrorKind::Domain,
                "current repository changed after the target-index seal plan",
                None,
            );
        }
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
    let repository = load_current(frontier);
    let assessment = assess_current(frontier, &repository);
    let codes = assessment_codes(&assessment, None);
    let changed_declared_paths = assessment
        .index
        .targets
        .iter()
        .filter(|target| {
            assessment
                .target_issues
                .get(&target.id)
                .is_some_and(|issues| !issues.is_empty())
        })
        .map(|target| target.packet.path.clone())
        .collect::<Vec<_>>();
    let report = TargetIndexRepairReport {
        schema: "vela.target-index-repair.v1",
        frontier_id: repository.frontier_id,
        index_schema: assessment.index.schema.clone(),
        index_root: assessment.index.index_root.clone(),
        historical_only: false,
        codes,
        changed_declared_paths,
        candidate_path: ".vela/tmp/target-index-candidate.json",
        generator_instruction: "Regenerate the closed domain-owned candidate and its packet outputs before running the seal check; Vela will not invent or repin target semantics.",
        repair_command: format!(
            "vela target-index seal {} --candidate .vela/tmp/target-index-candidate.json --check --json",
            frontier_display(frontier)
        ),
    };
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
    let repository = load_current(frontier);
    let repair_command = format!(
        "vela target-index repair {} --json",
        frontier_display(frontier)
    );
    let assessment = assess_current(frontier, &repository);
    let (summary, target) = if let Some(target_id) = target_id {
        validate_target_id(target_id)
            .unwrap_or_else(|error| ui::fail_with(ErrorKind::Usage, &error, None));
        let entry = assessment
            .index
            .targets
            .iter()
            .find(|target| target.id == target_id)
            .unwrap_or_else(|| {
                ui::fail_with(
                    ErrorKind::NotFound,
                    &format!("exact target ID {target_id:?} is absent from targets.json"),
                    Some(&repair_command),
                )
            });
        let codes = assessment_codes(&assessment, Some(target_id));
        let target = TargetIndexTargetInspection {
            schema: "vela.target-index-target-inspection.v1",
            index_schema: assessment.index.schema.clone(),
            index_root: assessment.index.index_root.clone(),
            target_id: entry.id.clone(),
            title: entry.title.clone(),
            state: entry.state.clone(),
            historical_only: false,
            actionable: entry.state == "open" && codes.is_empty(),
            codes,
            packet_ref: serde_json::to_value(&entry.packet).unwrap_or_else(|error| {
                ui::fail_with(ErrorKind::Internal, &error.to_string(), None)
            }),
            packet: assessment.packet_value(target_id).cloned(),
        };
        (None, Some(target))
    } else {
        let configured_open = assessment.configured_open();
        let summary = TargetIndexInspectionSummary {
            schema: "vela.target-index-inspection-summary.v1",
            frontier_id: repository.frontier_id.clone(),
            index_schema: assessment.index.schema.clone(),
            index_root: assessment.index.index_root.clone(),
            historical_only: false,
            configured_open,
            stale_open: configured_open.saturating_sub(assessment.fresh_open_targets().len()),
            codes: assessment_codes(&assessment, None),
            repair_command: repair_command.clone(),
        };
        (Some(summary), None)
    };
    let output = InspectOutput {
        schema: "vela.target-index-inspect.v1",
        ok: true,
        command: "target-index.inspect",
        frontier_id: repository.frontier_id,
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
