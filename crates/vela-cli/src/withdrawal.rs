use std::path::PathBuf;

pub(crate) fn cmd_proposal_withdraw(
    frontier: PathBuf,
    proposal_id: &str,
    actor: &str,
    reason: &str,
    json: bool,
) {
    if !actor.starts_with("agent:") {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Custody,
            "producer withdrawal requires the exact producer identity bound to this Proposal",
            None,
        );
    }
    let outcome = crate::workflow::transact_proposal_withdrawal(
        &frontier,
        proposal_id,
        actor,
        reason,
        || vela_edge::vela_agent_mcp::agent_signing_key(Some(actor)),
    )
    .unwrap_or_else(|error| crate::cli::fail_return(&error));
    if json {
        crate::cli::print_json(&outcome);
    } else {
        println!("withdrawn · {proposal_id}");
        println!(
            "  event: {}",
            outcome["withdrawal_event_id"].as_str().unwrap_or("unknown")
        );
        println!(
            "  publication: {}",
            outcome
                .get("publication")
                .map(ToString::to_string)
                .unwrap_or_else(|| "unchanged (idempotent)".to_string())
        );
    }
}
