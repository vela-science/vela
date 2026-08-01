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

fn execution_binding(
    packet_root: &str,
    packet: &Value,
) -> Result<Option<vela_protocol::execution_binding::ExecutionBindingV1>, String> {
    let Some(contracts) = packet.get("execution_contracts") else {
        return Ok(None);
    };
    let contracts = contracts
        .as_object()
        .ok_or_else(|| "Target packet execution_contracts must be an object".to_string())?;
    let root = |field: &str| -> Result<String, String> {
        let value = contracts
            .get(field)
            .and_then(|locator| locator.get("sha256"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!("Target packet execution_contracts.{field}.sha256 is missing")
            })?;
        if !vela_protocol::execution_binding::is_full_sha256_root(value) {
            return Err(format!(
                "Target packet execution_contracts.{field}.sha256 must be a full lowercase sha256 root"
            ));
        }
        Ok(value.to_string())
    };
    let binding = vela_protocol::execution_binding::ExecutionBindingV1 {
        schema: vela_protocol::execution_binding::EXECUTION_BINDING_SCHEMA.to_string(),
        packet_root: packet_root.to_string(),
        profile_root: root("producer_profile")?,
        verifier_capsule_root: root("verifier_capsule")?,
        result_contract_root: root("result_contract")?,
    };
    binding.validate()?;
    Ok(Some(binding))
}

fn briefing(frontier: &Path, target_id: &str) -> Result<Value, String> {
    let repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let profile = crate::current_repository::verify_current_profile_at(frontier)?;
    if profile.profile_root()? != repository.profile_root {
        return Err("current Frontier Profile does not match the repository profile root".into());
    }
    let assessment = vela_edge::target_index::assess_current_target_index(
        frontier,
        &repository.frontier_id,
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
            "current Target {target_id:?} is stale or invalid; run `vela check` for details"
        ));
    }
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
    let verifier = packet
        .get("verifier_profile")
        .or_else(|| packet.get("verifier"))
        .cloned();
    let execution_binding = execution_binding(&target.packet.sha256, &packet)?;

    let mut result = json!({
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
            "frontier_id": repository.frontier_id,
            "origin_id": repository.origin_id,
            "root": repository_root,
        },
        "target_index_root": assessment.index.index_root,
        "git": {
            "object_format": assessment.index.source.git_object_format,
            "commit": assessment.index.source.git_commit,
            "tree": assessment.index.source.git_tree,
        },
        "verifier": verifier,
        "authority_ceiling": AUTHORITY_CEILING,
    });
    if let Some(binding) = execution_binding {
        result
            .as_object_mut()
            .expect("start briefing is an object")
            .insert(
                "execution_binding".to_string(),
                serde_json::to_value(binding)
                    .map_err(|error| format!("serialize exact execution binding: {error}"))?,
            );
    }
    Ok(result)
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
            safe_text::inline(result["objective"].as_str().unwrap_or("unavailable"))
        );
        println!(
            "  packet    {}",
            safe_text::inline(result["packet_root"].as_str().unwrap_or("unavailable"))
        );
        if let Some(verifier) = result["verifier"].as_str() {
            println!("  verifier  {}", safe_text::inline(verifier));
        }
        if let Some(binding) = result["execution_binding"].as_object() {
            println!(
                "  profile   {}",
                safe_text::inline(binding["profile_root"].as_str().unwrap_or("unavailable"))
            );
            println!(
                "  capsule   {}",
                safe_text::inline(
                    binding["verifier_capsule_root"]
                        .as_str()
                        .unwrap_or("unavailable")
                )
            );
            println!(
                "  result    {}",
                safe_text::inline(
                    binding["result_contract_root"]
                        .as_str()
                        .unwrap_or("unavailable")
                )
            );
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

    #[test]
    fn rooted_packet_contracts_become_one_submission_binding() {
        let packet = json!({
            "execution_contracts": {
                "producer_profile": {"sha256": format!("sha256:{}", "2".repeat(64))},
                "verifier_capsule": {"sha256": format!("sha256:{}", "3".repeat(64))},
                "result_contract": {"sha256": format!("sha256:{}", "4".repeat(64))},
            }
        });
        let binding = execution_binding(&format!("sha256:{}", "1".repeat(64)), &packet)
            .unwrap()
            .unwrap();

        assert_eq!(binding.schema, "vela.execution-binding.v1");
        assert_eq!(binding.packet_root, format!("sha256:{}", "1".repeat(64)));
        assert_eq!(binding.profile_root, format!("sha256:{}", "2".repeat(64)));
        assert_eq!(
            binding.verifier_capsule_root,
            format!("sha256:{}", "3".repeat(64))
        );
        assert_eq!(
            binding.result_contract_root,
            format!("sha256:{}", "4".repeat(64))
        );
    }

    #[test]
    fn malformed_packet_contracts_fail_closed() {
        let packet = json!({
            "execution_contracts": {
                "producer_profile": {"sha256": "sha256:short"},
                "verifier_capsule": {"sha256": format!("sha256:{}", "3".repeat(64))},
                "result_contract": {"sha256": format!("sha256:{}", "4".repeat(64))},
            }
        });

        assert!(execution_binding(&format!("sha256:{}", "1".repeat(64)), &packet).is_err());
    }

    #[test]
    fn packets_without_execution_contracts_remain_valid() {
        assert_eq!(
            execution_binding(
                &format!("sha256:{}", "1".repeat(64)),
                &json!({"verifier_profile": "review-v1"}),
            )
            .unwrap(),
            None
        );
    }
}
