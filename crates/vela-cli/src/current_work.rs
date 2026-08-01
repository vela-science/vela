//! Stateless producer briefing for one exact current Target.
//!
//! `vela start` is a read-only orientation command. It verifies the current
//! repository and Target Index, constructs the exact producer-task binding,
//! and prints the information needed to author a bounded Submission. It does
//! not create a lease, Attempt, lock, counter, budget, or status record.

use std::path::Path;

use serde_json::{Value, json};

use crate::cli::safe_text;

const AUTHORITY_CEILING: &str =
    "Evidence may be submitted for review; only an authorized human Decision changes Standing.";

fn briefing(frontier: &Path, target_id: &str) -> Result<Value, String> {
    let repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let assessment = vela_edge::target_index::assess_current_target_index(
        frontier,
        &repository.frontier_id,
        &repository.origin_id,
        &repository_root,
    )?
    .ok_or_else(|| "current repository has no Target Index".to_string())?;
    let target = assessment
        .index
        .targets
        .iter()
        .find(|target| target.id == target_id)
        .cloned()
        .ok_or_else(|| format!("current Target Index has no target {target_id:?}"))?;
    let packet = assessment
        .packet_value(target_id)
        .cloned()
        .ok_or_else(|| format!("current Target {target_id:?} has no verified packet"))?;
    let binding = vela_edge::target_index::build_current_target_task_binding(
        frontier,
        &assessment,
        &repository.frontier_id,
        &repository.origin_id,
        &repository_root,
        target_id,
    )?;
    binding.validate()?;

    let submit_example = format!(
        "vela submit --frontier {} --claim <bounded-result> --type <type> --replayability <class> --artifact <path>:<kind> --caveat <limit> --as agent:<name> --json",
        frontier.display()
    );

    Ok(json!({
        "schema": "vela.start-briefing.v1",
        "ok": true,
        "command": "start",
        "frontier_id": repository.frontier_id,
        "target": target,
        "packet": packet,
        "binding": binding,
        "roots": {
            "repository": repository_root,
            "target_index": assessment.index.index_root,
            "git_commit": binding.claim_read_set.git_commit,
            "git_tree": binding.claim_read_set.git_tree,
            "packet": binding.packet.sha256,
            "binding": binding.binding_root,
        },
        "authority_ceiling": AUTHORITY_CEILING,
        "submit_example": submit_example,
        "writes": false,
    }))
}

pub(crate) fn cmd_start(frontier: &Path, target: &str, json_out: bool) {
    crate::ui::set_mode("start", json_out);
    let result = briefing(frontier, target).unwrap_or_else(|error| crate::cli::fail_return(&error));
    if json_out {
        crate::cli::print_json(&result);
    } else {
        println!(
            "start · {}",
            result["target"]["id"].as_str().unwrap_or(target)
        );
        println!(
            "  objective {}",
            safe_text::inline(
                result["target"]["objective"]
                    .as_str()
                    .unwrap_or("unavailable")
            )
        );
        println!(
            "  packet    {}",
            safe_text::inline(result["roots"]["packet"].as_str().unwrap_or("unavailable"))
        );
        println!(
            "  next      {}",
            safe_text::inline(
                result["submit_example"]
                    .as_str()
                    .unwrap_or("vela submit --help")
            )
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authority_ceiling_keeps_evidence_separate_from_standing() {
        assert!(AUTHORITY_CEILING.contains("Evidence"));
        assert!(AUTHORITY_CEILING.contains("human Decision"));
        assert!(AUTHORITY_CEILING.contains("Standing"));
    }
}
