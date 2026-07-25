//! Read-only compatibility surface for frozen Era-0 AcceptancePolicy records.
//!
//! Vela no longer authors, signs, rotates, revokes, or uses AcceptancePolicy
//! as a new authority surface. Historical policy bytes, signatures, heads, and
//! policy-lane events remain permanently verifiable. These commands expose
//! that retained state without mutating the frontier.

use std::path::Path;

use chrono::Utc;
use serde_json::json;
use vela_protocol::acceptance_policy::{
    AcceptancePolicy, ActivePolicySnapshot, Constraints, EVALUATOR_VERSION, Outcome, PolicyRule,
    evaluate, load_active_policy_snapshot,
};
use vela_protocol::cli_style as style;
use vela_protocol::project::Project;
use vela_protocol::proposals::policy_accept::{
    CAUSALLY_UNBOUNDED_POLICY_EXPIRY, POLICY_LANE_PAYLOAD_KEY, verify_policy_lane_events,
};
use vela_protocol::repo;

use crate::cli::print_json;
use crate::ui::{self, ErrorKind, fail_with};

struct EvaluationRow {
    proposal: String,
    kind: String,
    claim_class: String,
    outcome: Outcome,
    matched_rule_ids: Vec<String>,
    reasons: Vec<String>,
}

fn evaluate_pending(
    project: &Project,
    policy: &AcceptancePolicy,
    now: &str,
    frontier: Option<&Path>,
) -> Vec<EvaluationRow> {
    project
        .proposals
        .iter()
        .filter(|proposal| proposal.status == "pending_review")
        .map(|proposal| {
            let receipt = frontier.and_then(|frontier| {
                crate::review_material::frontier_receipt_for_proposal(frontier, proposal)
            });
            let context = crate::review_material::derive_existing_proposal_policy_context(
                frontier,
                Some(&policy.schema),
                project,
                &proposal.id,
                receipt.as_ref(),
                now,
            );
            let decision = evaluate(policy, &context, now);
            EvaluationRow {
                proposal: proposal.id.clone(),
                kind: proposal.kind.clone(),
                claim_class: context.claim_class,
                outcome: decision.outcome,
                matched_rule_ids: decision.matched_rule_ids,
                reasons: decision.reasons,
            }
        })
        .collect()
}

struct Admission {
    policy_id: String,
    event_id: String,
    proposal_id: String,
    rule_ids: Vec<String>,
    timestamp: String,
}

fn lane_admissions(project: &Project) -> Vec<Admission> {
    project
        .events
        .iter()
        .filter_map(|event| {
            let lane = event.payload.get(POLICY_LANE_PAYLOAD_KEY)?;
            Some(Admission {
                policy_id: lane
                    .get("policy_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                event_id: event.id.clone(),
                proposal_id: lane
                    .get("certificate")
                    .and_then(|certificate| certificate.get("proposal_id"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                rule_ids: lane
                    .get("rule_ids")
                    .and_then(|value| value.as_array())
                    .map(|rules| {
                        rules
                            .iter()
                            .filter_map(|rule| rule.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                timestamp: event.timestamp.clone(),
            })
        })
        .collect()
}

fn constraints_summary(constraints: &Constraints) -> String {
    let mut parts = vec![
        format!("A>={}", constraints.required_assurance_min),
        format!("changed<={}", constraints.max_changed_findings),
        format!("deps<={}", constraints.max_downstream_dependents),
    ];
    parts.push(if constraints.allow_semantic_text_change {
        "text change allowed".to_string()
    } else {
        "no text change".to_string()
    });
    if constraints.require_independence {
        parts.push("independence".to_string());
    }
    if constraints.require_method_integrity {
        parts.push("method integrity".to_string());
    }
    if constraints.allow_contested {
        parts.push("contested allowed".to_string());
    }
    if constraints.allow_governance_mutation {
        parts.push("governance allowed".to_string());
    }
    if let (Some(packet), Some(profile), Some(capsule), Some(result), Some(replayability)) = (
        constraints
            .allowed_packet_roots
            .as_ref()
            .and_then(|roots| roots.first()),
        constraints
            .allowed_profile_roots
            .as_ref()
            .and_then(|roots| roots.first()),
        constraints
            .allowed_verifier_capsule_roots
            .as_ref()
            .and_then(|roots| roots.first()),
        constraints
            .allowed_result_contract_roots
            .as_ref()
            .and_then(|roots| roots.first()),
        constraints.required_replayability.as_ref(),
    ) {
        let short = |root: &str| root.chars().take(19).collect::<String>();
        parts.push(format!(
            "exact packet={} profile={} capsule={} result={} replay={replayability}",
            short(packet),
            short(profile),
            short(capsule),
            short(result),
        ));
    }
    if let Some(credentials) = &constraints.allowed_producer_credential_roots {
        let short = |root: &str| root.chars().take(19).collect::<String>();
        parts.push(format!(
            "producer credential={}",
            credentials
                .iter()
                .map(|root| short(root))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    parts.join(" · ")
}

fn render_policy(policy: &AcceptancePolicy) {
    println!("  policy    {} · epoch {}", policy.id, policy.epoch);
    if !policy.frontier_id.is_empty() {
        println!("  frontier  {}", policy.frontier_id);
    }
    if !policy.issued_by.is_empty() {
        println!("  issued by {}", policy.issued_by.join(", "));
    }
    if policy.expires_at == CAUSALLY_UNBOUNDED_POLICY_EXPIRY {
        println!(
            "  default   {} · valid until signed rotation or revocation",
            policy.default.as_str()
        );
    } else {
        println!(
            "  default   {} · expires {} · Permit remains human-routed",
            policy.default.as_str(),
            policy.expires_at
        );
    }
    println!();
    println!("  rules");
    for rule in &policy.rules {
        render_rule(rule);
    }
}

fn render_rule(rule: &PolicyRule) {
    let classes = if rule.claim_classes.is_empty() {
        "any claim class".to_string()
    } else {
        rule.claim_classes.join(", ")
    };
    println!(
        "    {:<6} {}  →  {}",
        rule.effect.as_str(),
        rule.id,
        classes
    );
    if rule.effect == Outcome::Permit {
        println!(
            "           {}",
            style::dim(&constraints_summary(&rule.constraints))
        );
    }
}

fn print_admission_rows(rows: &[&Admission]) {
    for admission in rows {
        let rules = if admission.rule_ids.is_empty() {
            String::new()
        } else {
            format!("  ← {}", admission.rule_ids.join(", "))
        };
        println!(
            "    {}  {}  {}{rules}",
            admission.timestamp, admission.event_id, admission.proposal_id
        );
    }
}

pub(crate) fn cmd_policy_show(frontier: &Path, json: bool) {
    let snapshot = load_active_policy_snapshot(frontier);
    let project = repo::load_from_path(frontier).unwrap_or_else(|error| {
        fail_with(
            ErrorKind::Domain,
            &error,
            Some("run `vela check .` and repair the frontier before inspecting policy history"),
        )
    });
    let admissions = lane_admissions(&project);
    let now = Utc::now().to_rfc3339();
    let assessment = vela_protocol::proposals::policy_accept::assess_policy_readiness(
        &project,
        snapshot.as_ref().map_err(String::as_str),
        &now,
    );
    if assessment.permit_readiness()
        == vela_protocol::proposals::policy_accept::PermitReadiness::Blocked
    {
        let parsed_policy_id = snapshot
            .as_ref()
            .ok()
            .and_then(|snapshot| snapshot.policy())
            .map(|policy| policy.id.as_str());
        let retained: Vec<&Admission> = admissions
            .iter()
            .filter(|admission| {
                parsed_policy_id.is_none_or(|policy_id| admission.policy_id == policy_id)
            })
            .collect();
        let last: Vec<&Admission> = retained.iter().rev().take(5).rev().copied().collect();
        if json {
            let policy = snapshot
                .as_ref()
                .ok()
                .and_then(|snapshot| snapshot.policy())
                .and_then(|policy| serde_json::to_value(policy).ok())
                .unwrap_or(serde_json::Value::Null);
            let signature = snapshot
                .as_ref()
                .ok()
                .and_then(|snapshot| snapshot.verified.as_ref())
                .map(|verified| {
                    json!({
                        "signer_pubkey_hex": verified.signer_pubkey_hex,
                        "signed_at": verified.signed_at,
                    })
                });
            print_json(&json!({
                "ok": false,
                "command": "policy.show",
                "state": assessment.state().as_str(),
                "permit_readiness": assessment.permit_readiness().as_str(),
                "reason_codes": assessment.reason_codes(),
                "error": assessment.detail(),
                "policy": policy,
                "signature": signature,
                "admissions": {
                    "scope": if parsed_policy_id.is_some() {
                        "parsed_policy"
                    } else {
                        "all_retained_policy_lanes"
                    },
                    "count": retained.len(),
                    "last": last.iter().map(|admission| json!({
                        "policy_id": admission.policy_id,
                        "event": admission.event_id,
                        "proposal": admission.proposal_id,
                        "rule_ids": admission.rule_ids,
                        "timestamp": admission.timestamp,
                    })).collect::<Vec<_>>(),
                },
            }));
        } else {
            ui::header("POLICY", &frontier.display().to_string(), None);
            println!(
                "  {}",
                style::lost(&format!(
                    "{} · Permit blocked ({})",
                    assessment.state().as_str(),
                    assessment.reason_codes().join(", ")
                ))
            );
            if let Some(detail) = assessment.detail() {
                println!("  {detail}");
            }
            println!();
            println!("  frozen Era-0 policy history remains retained and fail-closed.");
            if retained.is_empty() {
                println!("  retained policy-lane admissions: none");
            } else {
                println!(
                    "  retained policy-lane admissions: {} event(s), last {}:",
                    retained.len(),
                    last.len()
                );
                for admission in last {
                    println!(
                        "    {}  {}  {}  {}",
                        admission.timestamp,
                        admission.event_id,
                        admission.proposal_id,
                        admission.policy_id
                    );
                }
            }
        }
        std::process::exit(1);
    }
    let snapshot = snapshot.expect("non-blocked assessment has a valid snapshot");
    if assessment.state() == vela_protocol::proposals::policy_accept::PolicyState::Absent {
        if json {
            print_json(&json!({
                "ok": true,
                "command": "policy.show",
                "state": assessment.state().as_str(),
                "permit_readiness": assessment.permit_readiness().as_str(),
                "reason_codes": assessment.reason_codes(),
                "policy": serde_json::Value::Null,
                "signature": serde_json::Value::Null,
                "admissions": { "count": 0, "last": [] },
            }));
        } else {
            ui::header("POLICY", &frontier.display().to_string(), None);
            println!("  absent · Permit human_only (policy_absent)");
            println!("  new authority requires the repository-authority migration.");
        }
        return;
    }
    let policy = snapshot
        .policy()
        .expect("a non-absent policy snapshot retains its parsed policy");
    let signed = snapshot.verified.as_ref();
    let expired = policy.is_expired(&now);
    let mine: Vec<&Admission> = admissions
        .iter()
        .filter(|admission| admission.policy_id == policy.id)
        .collect();
    let last: Vec<&Admission> = mine.iter().rev().take(5).rev().copied().collect();

    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.show",
            "state": assessment.state().as_str(),
            "permit_readiness": assessment.permit_readiness().as_str(),
            "reason_codes": assessment.reason_codes(),
            "readiness_detail": assessment.detail(),
            "expired": expired,
            "policy": serde_json::to_value(policy).unwrap_or_default(),
            "signature": signed.map(|verified| json!({
                "signer_pubkey_hex": verified.signer_pubkey_hex,
                "signed_at": verified.signed_at,
            })),
            "admissions": {
                "count": mine.len(),
                "last": last.iter().map(|admission| json!({
                    "event": admission.event_id,
                    "proposal": admission.proposal_id,
                    "rule_ids": admission.rule_ids,
                    "timestamp": admission.timestamp,
                })).collect::<Vec<_>>(),
            },
        }));
        return;
    }

    ui::header(
        "POLICY",
        &frontier.display().to_string(),
        Some("frozen Era-0"),
    );
    render_policy(policy);
    println!();
    let state_line = format!(
        "{} · Permit {}{}",
        assessment.state().as_str(),
        assessment.permit_readiness().as_str(),
        if assessment.reason_codes().is_empty() {
            String::new()
        } else {
            format!(" ({})", assessment.reason_codes().join(", "))
        }
    );
    match assessment.permit_readiness() {
        vela_protocol::proposals::policy_accept::PermitReadiness::Ready => {
            println!("  {}", style::ok(&state_line));
        }
        vela_protocol::proposals::policy_accept::PermitReadiness::HumanOnly => {
            println!("  {}", style::warn(&state_line));
        }
        vela_protocol::proposals::policy_accept::PermitReadiness::Blocked => {
            println!("  {}", style::lost(&state_line));
        }
    }
    if let Some(signature) = signed {
        println!(
            "  signature verified: {}… at {}",
            &signature.signer_pubkey_hex[..16.min(signature.signer_pubkey_hex.len())],
            signature.signed_at
        );
    }
    if expired {
        println!(
            "  {} expired at {} — Permit remains human_only",
            style::lost("note"),
            policy.expires_at
        );
    }
    println!();
    if mine.is_empty() {
        println!("  admitted under this policy: nothing");
    } else {
        println!(
            "  admitted under this policy: {} event(s), last {}:",
            mine.len(),
            last.len()
        );
        print_admission_rows(&last);
    }
}

pub(crate) fn cmd_policy_test(frontier: &Path, json: bool) {
    let spin = (!json).then(|| {
        crate::cli::progress::Spinner::start(
            "replaying the frozen policy over retained pending proposals",
        )
    });
    let project = repo::load_from_path(frontier)
        .unwrap_or_else(|error| fail_with(ErrorKind::Domain, &error, None));
    let now = Utc::now().to_rfc3339();
    let snapshot = load_active_policy_snapshot(frontier);
    let assessment = vela_protocol::proposals::policy_accept::assess_policy_readiness(
        &project,
        snapshot.as_ref().map_err(String::as_str),
        &now,
    );
    if assessment.permit_readiness()
        == vela_protocol::proposals::policy_accept::PermitReadiness::Blocked
    {
        fail_with(
            ErrorKind::Domain,
            assessment
                .detail()
                .unwrap_or("frozen policy readiness assessment is blocked"),
            Some("inspect the retained policy pair and causal policy-head chain"),
        );
    }
    let snapshot = snapshot.expect("a non-blocked policy assessment has a valid snapshot");
    let policy = snapshot.policy().unwrap_or_else(|| {
        fail_with(
            ErrorKind::NotFound,
            "no frozen policy at .vela/policies/active.json",
            Some("new authority requires the repository-authority migration"),
        )
    });
    let rows = evaluate_pending(&project, policy, &now, Some(frontier));
    let permit = rows
        .iter()
        .filter(|row| row.outcome == Outcome::Permit)
        .count();
    let defer = rows
        .iter()
        .filter(|row| row.outcome == Outcome::Defer)
        .count();
    let deny = rows
        .iter()
        .filter(|row| row.outcome == Outcome::Deny)
        .count();

    if let Some(spinner) = spin {
        spinner.finish("replayed");
    }
    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.test",
            "evaluation": "frozen_era0_replay",
            "policy_id": policy.id,
            "state": assessment.state().as_str(),
            "permit_readiness": assessment.permit_readiness().as_str(),
            "reason_codes": assessment.reason_codes(),
            "evaluator": EVALUATOR_VERSION,
            "now": now,
            "summary": { "permit": permit, "defer": defer, "deny": deny, "total": rows.len() },
            "decisions": rows.iter().map(|row| json!({
                "proposal": row.proposal,
                "kind": row.kind,
                "claim_class": row.claim_class,
                "outcome": row.outcome.as_str(),
                "matched_rules": row.matched_rule_ids,
                "reasons": row.reasons,
            })).collect::<Vec<_>>(),
        }));
        return;
    }

    ui::header("POLICY", &policy.id, Some("frozen Era-0 replay"));
    println!(
        "  {} pending proposal(s): {permit} permit, {defer} defer, {deny} deny",
        rows.len()
    );
    println!();
    for row in rows.iter().take(20) {
        let reason = if row.outcome == Outcome::Permit {
            row.matched_rule_ids.first().cloned().unwrap_or_default()
        } else {
            row.reasons.first().cloned().unwrap_or_default()
        };
        println!(
            "  [{}] {} ({}, {})  ← {reason}",
            row.outcome.as_str(),
            row.proposal,
            row.kind,
            row.claim_class
        );
    }
    if rows.len() > 20 {
        println!("  … {} more (use --json for all)", rows.len() - 20);
    }
    println!();
    println!("  compatibility replay only — no policy or proposal was changed.");
}

fn parse_sidon_bound(text: &str) -> Option<(i64, i64)> {
    let index = text.find("a(")?;
    let rest = &text[index + 2..];
    let close = rest.find(')')?;
    let n: i64 = rest[..close].trim().parse().ok()?;
    let after = &rest[close + 1..];
    let greater_equal = after.find(">=")?;
    let tail = &after[greater_equal + 2..];
    let digits: String = tail
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    Some((n, digits.parse().ok()?))
}

fn best_sidon_bound_for_n(project: &Project, n: i64, exclude_finding: Option<&str>) -> Option<i64> {
    let mut best = None;
    for finding in &project.findings {
        if Some(finding.id.as_str()) == exclude_finding {
            continue;
        }
        if let Some((finding_n, finding_k)) = parse_sidon_bound(&finding.assertion.text)
            && finding_n == n
        {
            best = Some(best.map_or(finding_k, |current: i64| current.max(finding_k)));
        }
    }
    best
}

pub(crate) fn cmd_policy_evaluate_proposal(frontier: &Path, proposal_id: &str, json: bool) {
    let project = repo::load_from_path(frontier).unwrap_or_else(|error| {
        fail_with(
            ErrorKind::NotFound,
            &format!("cannot load frontier: {error}"),
            None,
        )
    });
    let admission = lane_admissions(&project)
        .into_iter()
        .find(|admission| admission.proposal_id == proposal_id);
    let lane_errors = verify_policy_lane_events(&project, frontier);
    let (admitted, rule_ids, admit_event, replay_error) = match &admission {
        Some(admission) => {
            let error = lane_errors
                .iter()
                .find(|error| error.starts_with(&admission.event_id))
                .cloned();
            (
                error.is_none(),
                admission.rule_ids.clone(),
                Some(admission.event_id.clone()),
                error,
            )
        }
        None => (false, Vec::new(), None, None),
    };
    let verdict = if admitted { "permit" } else { "defer" };
    let claim_text = project
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .and_then(|proposal| {
            proposal
                .payload
                .pointer("/finding/assertion/text")
                .or_else(|| proposal.payload.pointer("/assertion/text"))
                .or_else(|| proposal.payload.pointer("/text"))
                .and_then(|text| text.as_str())
                .map(str::to_string)
        });
    let own_finding_id = admit_event.as_ref().and_then(|event_id| {
        project
            .events
            .iter()
            .find(|event| &event.id == event_id)
            .map(|event| event.target.id.clone())
    });
    let bound = claim_text.as_deref().and_then(parse_sidon_bound);
    let (n, claimed) = bound.map_or((None, None), |(n, claimed)| (Some(n), Some(claimed)));
    let current_best = n.and_then(|dimension| {
        best_sidon_bound_for_n(&project, dimension, own_finding_id.as_deref())
    });
    let is_beat = match (claimed, current_best) {
        (Some(value), Some(best)) => value > best,
        (Some(_), None) => true,
        _ => false,
    };

    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.evaluate-proposal",
            "mode": "frozen_era0_replay",
            "frontier": frontier.display().to_string(),
            "proposal_id": proposal_id,
            "admitted": admitted,
            "verdict": verdict,
            "rule_ids": rule_ids,
            "n": n,
            "claimed": claimed,
            "current_best": current_best,
            "is_beat": is_beat,
            "replay_error": replay_error,
        }));
        return;
    }

    println!("policy · retained admission · {proposal_id}");
    let rule = if rule_ids.is_empty() {
        String::new()
    } else {
        format!(", rule {}", rule_ids.join(","))
    };
    println!("  admitted: {admitted} (verdict: {verdict}{rule})");
    match (n, claimed, current_best) {
        (Some(n), Some(value), Some(best)) => println!(
            "  historical beat check: a({n}) >= {value} vs {best} → {}",
            if is_beat { "beats" } else { "not a beat" }
        ),
        (Some(n), Some(value), None) => {
            println!("  historical beat check: a({n}) >= {value} → first record")
        }
        _ => println!("  historical beat check: not a parseable Sidon bound"),
    }
    if let Some(error) = replay_error {
        println!("  {} replay check: {error}", style::warn("!"));
    }
}

pub(crate) fn cmd_policy_log(frontier: &Path, json: bool) {
    let project = repo::load_from_path(frontier)
        .unwrap_or_else(|error| fail_with(ErrorKind::Domain, &error, None));
    let snapshot = load_active_policy_snapshot(frontier);
    let assessment = vela_protocol::proposals::policy_accept::assess_policy_readiness(
        &project,
        snapshot.as_ref().map_err(String::as_str),
        &Utc::now().to_rfc3339(),
    );
    let current_policy_id = snapshot
        .as_ref()
        .ok()
        .and_then(ActivePolicySnapshot::policy)
        .map(|policy| policy.id.as_str());
    let admissions = lane_admissions(&project);
    let mut by_policy: std::collections::BTreeMap<String, Vec<&Admission>> =
        std::collections::BTreeMap::new();
    for admission in &admissions {
        by_policy
            .entry(admission.policy_id.clone())
            .or_default()
            .push(admission);
    }

    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.log",
            "mode": "frozen_era0_history",
            "policy_state": assessment.state().as_str(),
            "permit_readiness": assessment.permit_readiness().as_str(),
            "reason_codes": assessment.reason_codes(),
            "current_policy_id": current_policy_id,
            "total": admissions.len(),
            "policies": by_policy.iter().map(|(policy_id, rows)| json!({
                "policy_id": policy_id,
                "current": Some(policy_id.as_str()) == current_policy_id,
                "count": rows.len(),
                "admissions": rows.iter().map(|admission| json!({
                    "event": admission.event_id,
                    "proposal": admission.proposal_id,
                    "rule_ids": admission.rule_ids,
                    "timestamp": admission.timestamp,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }));
        return;
    }

    ui::header(
        "POLICY",
        &frontier.display().to_string(),
        Some("frozen Era-0 admissions"),
    );
    println!(
        "  current policy {} · Permit {}{}",
        assessment.state().as_str(),
        assessment.permit_readiness().as_str(),
        if assessment.reason_codes().is_empty() {
            String::new()
        } else {
            format!(" ({})", assessment.reason_codes().join(", "))
        }
    );
    if by_policy.is_empty() {
        println!();
        println!("  no retained policy-lane admissions");
        return;
    }
    println!(
        "  {} admission(s) under {} frozen policy epoch(s)",
        admissions.len(),
        by_policy.len()
    );
    for (policy_id, rows) in &by_policy {
        let current = if Some(policy_id.as_str()) == current_policy_id {
            format!("  {}", style::live("current"))
        } else {
            String::new()
        };
        println!();
        println!("  {policy_id} — {} admission(s){current}", rows.len());
        print_admission_rows(rows);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_sidon_bound;

    #[test]
    fn parses_only_explicit_sidon_lower_bounds() {
        assert_eq!(
            parse_sidon_bound("A witness establishes a(24) >= 7194."),
            Some((24, 7194))
        );
        assert_eq!(parse_sidon_bound("a(24) > 7194"), None);
        assert_eq!(parse_sidon_bound("unrelated claim"), None);
    }
}
