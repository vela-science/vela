//! Stateless producer briefing for one exact current Target.
//!
//! `vela start` is a read-only orientation command. It verifies the current
//! repository and Target Index, then prints the exact information needed to
//! author a bounded Submission. It does not create or retain execution state.

use std::path::Path;

use serde_json::{Value, json};

use crate::cli::safe_text;

const AUTHORITY_CEILING: &str =
    "Evidence may be submitted for review; only an authorized human Decision changes Standing.";

fn briefing(frontier: &Path, target_id: &str) -> Result<Value, String> {
    let repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let profile = crate::current_repository::verify_current_profile_at(frontier)?;
    if profile.profile_root()? != repository.profile_root {
        return Err(
            "retained repository profile does not match the repository profile root".into(),
        );
    }
    let assessment = vela_edge::target_index::assess_current_target_index(
        frontier,
        &repository.repository_id,
        &repository.origin_id,
        &repository_root,
    )?
    .ok_or_else(|| "current repository has no Target Index".to_string())?;
    if !assessment.global_issues.is_empty()
        || assessment
            .target_issues
            .get(target_id)
            .is_some_and(|issues| !issues.is_empty())
    {
        return Err(format!(
            "current Target {target_id:?} is stale or invalid; run `vela replay` for details"
        ));
    }
    let target = assessment
        .index
        .targets
        .iter()
        .find(|target| target.id == target_id)
        .cloned()
        /* The only miss in this function: every other failure above is a stale
        or unverifiable index, which is a domain failure, not a bad argument. */
        .unwrap_or_else(|| {
            crate::cli::fail_kind_return(
                crate::ui::ErrorKind::NotFound,
                &format!("current Target Index has no target {target_id:?}"),
            )
        });
    let packet = assessment
        .packet_value(target_id)
        .cloned()
        .ok_or_else(|| format!("current Target {target_id:?} has no verified packet"))?;
    let verifier = packet
        .get("verifier_profile")
        .or_else(|| packet.get("verifier"))
        .cloned();
    let result = json!({
        "schema": "vela.start-briefing.v2",
        "target": {
            "id": target.id,
            "title": target.title,
        },
        "objective": target.objective,
        "scope": profile.scope,
        "packet": packet,
        "packet_root": target.packet.sha256,
        "repository": {
            "repository_id": repository.repository_id,
            "origin_id": repository.origin_id,
            "root": repository_root,
        },
        "target_index_root": assessment.index.index_root,
        "git": {
            "role": "target_index_source",
            "object_format": assessment.index.source.git_object_format,
            "commit": assessment.index.source.git_commit,
            "tree": assessment.index.source.git_tree,
        },
        "verifier": verifier,
        "authority_ceiling": AUTHORITY_CEILING,
    });
    Ok(result)
}

pub(crate) fn cmd_start(frontier: &Path, target: &str, json_out: bool) {
    crate::ui::set_mode("start", json_out);
    crate::ui::require_initialized_repo(frontier);
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
            safe_text::inline(result["objective"].as_str().unwrap_or("unavailable"))
        );
        println!(
            "  packet    {}",
            safe_text::inline(result["packet_root"].as_str().unwrap_or("unavailable"))
        );
        if let Some(verifier) = result["verifier"].as_str() {
            println!("  verifier  {}", safe_text::inline(verifier));
        }
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
