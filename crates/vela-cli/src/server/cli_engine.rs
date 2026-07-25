use crate::cli::{
    active_policy_pair_snapshot, collect_witness_files, fail, fail_return, parse_witness,
    print_json,
};
use crate::cli_commands::*;
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};
use vela_protocol::cli_style as style;
use vela_protocol::evidence_ci;
use vela_protocol::repo;

pub(crate) fn cmd_verify_evidence(action: VerifyAction) {
    match action {
        VerifyAction::Attach {
            frontier,
            attachment,
            proposal,
            actor,
            json,
        } => {
            crate::ui::set_mode("verify.attach", json);
            let bytes = std::fs::read(&attachment).unwrap_or_else(|error| {
                fail_return(&format!("read {}: {error}", attachment.display()))
            });
            let record: vela_protocol::verifier_attachment::VerifierAttachment =
                serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                    fail_return(&format!(
                        "parse {} as VerifierAttachment: {error}",
                        attachment.display()
                    ))
                });
            let result =
                crate::workflow::attach_proposal_verifier(&frontier, &proposal, record, &actor)
                    .unwrap_or_else(|error| fail_return(&error));
            if json {
                print_json(&result);
            } else {
                println!(
                    "verify attach: retained {} for proposal {}",
                    result["attachment_id"].as_str().unwrap_or("attachment"),
                    proposal
                );
                println!("  acceptance: unchanged (delta 0)");
                println!(
                    "  gate: {}",
                    result
                        .pointer("/gate/status")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                );
                if let Some(next) = result["next_missing_condition"].as_str() {
                    println!("  next: {next}");
                }
            }
        }
    }
}

pub(crate) fn cmd_gate(action: GateAction) {
    use vela_edge::deliverable_grade::{self, DeliverableGrade, GradeGate};
    use vela_protocol::verifier_attachment::{
        self, GateStatus, ProbeKind, VerifierAttachment, VerifierMethod,
    };
    match action {
        GateAction::Grade { claim, grade, json } => {
            let gate = deliverable_grade::grade_gate(&claim, grade.as_deref());
            let passed = gate.passed();
            if json {
                let grade_str = match &gate {
                    GradeGate::Ok(g) => Some(g.as_str().to_string()),
                    _ => grade.clone(),
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "command": "gate grade",
                        "passed": passed,
                        "grade": grade_str,
                        "reason": gate.reason(),
                    }))
                    .expect("serialize gate grade response")
                );
            } else if passed {
                println!("gate grade: ok");
                if let GradeGate::Ok(g) = gate {
                    println!("  deliverable_grade: {g}  (claim text consistent with grade)");
                }
            } else {
                eprintln!("gate grade: REJECTED\n  {}", gate.reason());
            }
            if !passed {
                std::process::exit(1);
            }
        }
        GateAction::Check {
            claim,
            attachments,
            json,
        } => {
            let raw = std::fs::read_to_string(&attachments)
                .unwrap_or_else(|e| fail_return(&format!("read {}: {e}", attachments.display())));
            let atts: Vec<VerifierAttachment> = serde_json::from_str(&raw).unwrap_or_else(|e| {
                fail_return(&format!(
                    "parse {} as a JSON array of VerifierAttachment: {e}",
                    attachments.display()
                ))
            });
            // G4: every attachment must be structurally sound before the
            // gate reasons over it.
            for a in &atts {
                if let Err(e) = a.verify() {
                    fail(&format!("attachment {} is malformed: {e}", a.id));
                }
            }
            let digest = verifier_attachment::claim_digest(&claim);
            let outcome = verifier_attachment::derive_gate_status(&digest, &atts);
            let verified = outcome.status == GateStatus::Verified;
            if json {
                let status = match outcome.status {
                    GateStatus::Verified => "verified",
                    GateStatus::NeedsVerification => "needs_verification",
                    GateStatus::Refuted => "refuted",
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "command": "gate check",
                        "claim_digest": digest,
                        "attachments": atts.len(),
                        "status": status,
                        "reasons": outcome.reasons,
                    }))
                    .expect("serialize gate check response")
                );
            } else {
                println!(
                    "gate check: {} attachment(s) over claim {digest}",
                    atts.len()
                );
                match outcome.status {
                    GateStatus::Verified => println!(
                        "  status: VERIFIED\n  >=2 independent matched attachments + a surviving adversarial probe."
                    ),
                    GateStatus::Refuted => {
                        println!("  status: REFUTED");
                        for r in &outcome.reasons {
                            println!("    - {r}");
                        }
                    }
                    GateStatus::NeedsVerification => {
                        println!("  status: needs_verification");
                        for r in &outcome.reasons {
                            println!("    - {r}");
                        }
                    }
                }
            }
            if !verified {
                std::process::exit(1);
            }
        }
        GateAction::Vocab { json } => {
            let grades: Vec<&str> = DeliverableGrade::ALL.iter().map(|g| g.as_str()).collect();
            let methods: Vec<&str> = VerifierMethod::ALL.iter().map(|m| m.as_str()).collect();
            let probes = [
                ProbeKind::CounterexampleSearch,
                ProbeKind::CaseBConfig,
                ProbeKind::BoundaryDualFeasibility,
                ProbeKind::FiniteSizeExtrapolation,
                ProbeKind::IndependentReimplementation,
                ProbeKind::FormalismFidelity,
            ];
            let probe_kinds: Vec<&str> = probes.iter().map(|p| p.as_str()).collect();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "command": "gate vocab",
                        "deliverable_grades": grades,
                        "solve_grades": DeliverableGrade::ALL
                            .iter()
                            .filter(|g| g.is_solve())
                            .map(|g| g.as_str())
                            .collect::<Vec<_>>(),
                        "verifier_methods": methods,
                        "probe_kinds": probe_kinds,
                    }))
                    .expect("serialize gate vocab response")
                );
            } else {
                println!("deliverable grades ({}):", grades.len());
                for g in DeliverableGrade::ALL {
                    let mark = if g.is_solve() { " (solve)" } else { "" };
                    println!("  {}{mark}", g.as_str());
                }
                println!("\nverifier methods ({}):", methods.len());
                for m in &methods {
                    println!("  {m}");
                }
                println!("\nadversarial probe kinds ({}):", probe_kinds.len());
                for p in &probe_kinds {
                    println!("  {p}");
                }
            }
        }
    }
}

/// Compute the exact-lane policy route for one proposed finding.
///
/// The floor (un-forgeable, agent cannot fake): (1) a fresh `vela-verify`
/// re-check of the finding's witness, computed here, not trusted from a field;
/// (2) the frozen `claim_witness_faithful` binding the parsed assertion to the
/// witness structure. Receipt landing calls this shared evaluator before the
/// signed policy can route the proposal; it never writes by itself.
pub(crate) struct PolicyRouteVerdict {
    pub would_permit: bool,
    pub canonical_claim: Option<String>,
}

/// Compute the exact-lane policy verdict without writing. This is the one place
/// the frozen floor is evaluated for Receipt routing.
pub(crate) fn evaluate_exact_policy_route(
    frontier: &Path,
    finding_id: &str,
) -> Result<PolicyRouteVerdict, String> {
    use std::collections::BTreeSet;

    let source = repo::detect(frontier)?;
    let proj = repo::load(&source)?;

    // Resolve the finding: a landed canonical finding, or a pending finding.add
    // proposal's payload. Both carry the assertion text + provenance the floor
    // and guards read.
    let (finding, proposal) = resolve_finding_and_proposal(&proj, finding_id);
    let finding = finding.ok_or_else(|| {
        format!("no finding '{finding_id}' (landed or in a pending finding.add proposal)")
    })?;
    let proposal = proposal.ok_or_else(|| {
        format!(
            "no finding.add proposal targets '{finding_id}'; the exact lane admits a proposal, \
             not an already-landed finding"
        )
    })?;

    // FLOOR step 1: a fresh frozen re-check of the finding's witness.
    let (witness_ok, _witness_msg, witness) =
        reproduce_finding_witness(&proj, frontier, finding_id);
    // FLOOR step 2: frozen claim<->witness faithfulness.
    let faithful = witness
        .as_ref()
        .map(|w| vela_verify::claim_witness_faithful(&finding.assertion.text, w));
    // The canonical, WITNESS-DERIVED verified claim: exactly what an admit
    // establishes, independent of the author's prose. Surfaces should display
    // THIS (not the assertion text) as the machine_verified claim so prose
    // cannot puff a true bound. (docs/VERIFICATION.md §8 residual.)
    let canonical_claim = witness.as_ref().and_then(vela_verify::canonical_claim);

    // Proposal-level guard inputs, derived live (never trusted from a field).
    let synthetic: BTreeSet<String> = proj
        .findings
        .iter()
        .filter(|f| is_synthetic_source(&f.provenance.source_type))
        .map(|f| f.id.clone())
        .collect();
    let mut synthetic = synthetic;
    if is_synthetic_source(&finding.provenance.source_type) {
        synthetic.insert(finding.id.clone());
    }
    let graph = vela_protocol::frontier_graph::FrontierGraph::from_project(&proj);
    let open_contradictions: BTreeSet<String> = vela_protocol::contradiction::derive_candidates(
        &graph,
        proj.frontier_id.as_deref().unwrap_or_default(),
    )
    .into_iter()
    .filter(|c| c.is_open())
    .flat_map(|c| [c.finding_a.clone(), c.finding_b.clone()])
    .collect();
    let matched: Vec<_> = proj
        .verifier_attachments
        .iter()
        .filter(|a| a.target == finding.id)
        .cloned()
        .collect();

    // The proposal-level wrapper (kind, target, drift-pin, lifecycle, synthetic,
    // contradiction, producer != verifier, then the attachment predicate UNLESS
    // floor-sufficient). For the exact lane, the FLOOR (a fresh frozen reproduce
    // + claim_witness_faithful binding) IS the proof: when faithfulness binds,
    // the >=2-independent-attachment bar (the general gate's, for claims with no
    // single frozen verifier) is waived. The witness genuinely reproducing +
    // structurally establishing the parsed claim is a complete, un-forgeable
    // proof of an exact lower-bound/size claim.
    let floor_ok = witness_ok && faithful.as_ref().map(|f| f.faithful).unwrap_or(false);
    let (wrapper_ok, _wrapper_reasons) = vela_protocol::proposals::exact_lane_eligible(
        &proposal,
        &finding,
        &matched,
        &open_contradictions,
        &synthetic,
        floor_ok,
    );

    // Guard #3 (attachment provenance), scoped to where attachments are
    // actually load-bearing — see `attachment_vouch_gate`.
    let (vouched_ok, _vouch_reason) = attachment_vouch_gate(floor_ok, matched.len());

    let mut would_permit = floor_ok && wrapper_ok && vouched_ok;

    // The sealed, SIGNED acceptance policy (when present) is the governing
    // authority over this lane: it can only TIGHTEN the frozen floor above
    // (a Defer/Deny verdict refuses an admit the floor would allow; a
    // Permit never overrides a failed floor). Signer must be a registered
    // human reviewer on the frontier. Absent policy = today's behavior.
    let policy_snapshot = vela_protocol::acceptance_policy::load_active_policy_snapshot(frontier);
    let now = chrono::Utc::now().to_rfc3339();
    let policy_assessment = vela_protocol::proposals::policy_accept::assess_policy_readiness(
        &proj,
        policy_snapshot.as_ref().map_err(String::as_str),
        &now,
    );
    if policy_assessment.permit_readiness()
        == vela_protocol::proposals::policy_accept::PermitReadiness::Blocked
    {
        return Err(format!(
            "active policy: {}",
            policy_assessment
                .detail()
                .unwrap_or("policy readiness assessment is blocked")
        ));
    }
    match policy_snapshot {
        Ok(snapshot) => {
            if let Some(vp) = snapshot.verified {
                // This legacy audit lane has no Receipt v1 body binding or
                // frontier-resolved producer credential. Feed those unknowns to
                // the conservative policy context instead of manufacturing the
                // old `credential_valid=true`/`has_unknown_fields=false` pair.
                // The frozen floor may still drive its non-authoritative audit
                // event when no signed policy is active; it cannot satisfy a live
                // policy with facts this path does not possess.
                let ctx = vela_protocol::acceptance_policy::PolicyContext {
                    claim_class: vela_protocol::proposals::policy_accept::proposal_claim_class(
                        &proposal,
                    ),
                    ..vela_protocol::acceptance_policy::PolicyContext::default()
                };
                let decision = vela_protocol::acceptance_policy::evaluate(&vp.policy, &ctx, &now);
                let permitted = decision.outcome
                    == vela_protocol::acceptance_policy::Outcome::Permit
                    && policy_assessment.permit_readiness()
                        == vela_protocol::proposals::policy_accept::PermitReadiness::Ready;
                would_permit = would_permit && permitted;
            }
        }
        Err(error) => return Err(format!("active policy: {error}")),
    }

    Ok(PolicyRouteVerdict {
        would_permit,
        canonical_claim,
    })
}

/// Resolve a finding by id from canonical state or a pending finding.add
/// proposal payload, returning the finding and the finding.add proposal that
/// carries it (the exact lane admits a proposal, so the proposal is required).
fn resolve_finding_and_proposal(
    proj: &vela_protocol::project::Project,
    finding_id: &str,
) -> (
    Option<vela_protocol::bundle::FindingBundle>,
    Option<vela_protocol::proposals::StateProposal>,
) {
    let proposal = proj
        .proposals
        .iter()
        .find(|p| {
            p.kind == "finding.add"
                && (p.target.id == finding_id
                    || p.payload
                        .get("finding")
                        .and_then(|f| f.get("id"))
                        .and_then(|i| i.as_str())
                        == Some(finding_id))
        })
        .cloned();
    // Prefer the proposal's own finding body (what the lane admits); fall back
    // to the landed finding.
    let finding = proposal
        .as_ref()
        .and_then(|p| p.payload.get("finding").cloned())
        .and_then(|v| serde_json::from_value::<vela_protocol::bundle::FindingBundle>(v).ok())
        .or_else(|| proj.findings.iter().find(|f| f.id == finding_id).cloned());
    (finding, proposal)
}

/// True if a provenance source_type denotes a synthetic NARRATIVE source that
/// needs human review (mirrors the `synthetic_source_requires_review` signal,
/// signals.rs). Deliberately NOT `model_output`: a campaign produces a finding
/// whose trust is its frozen WITNESS (the floor re-checks it), so the producer
/// being a model is exactly what the floor handles — the positive provenance is
/// the reproduce-clean witness, not the prose source. Only a synthetic report /
/// agent trace with no frozen witness is the thing this guard catches.
fn is_synthetic_source(source_type: &str) -> bool {
    let s = source_type.trim().to_ascii_lowercase();
    s == "synthetic_report" || s == "agent_trace"
}

/// Guard #3 (attachment provenance), scoped to where attachments are actually
/// load-bearing. The exact lane's trust is the FLOOR: a fresh frozen `vela
/// reproduce` over the witness plus `claim_witness_faithful` binding the parsed
/// claim to the witness structure. When the floor holds it is a complete,
/// un-forgeable proof of the exact lower-bound/size claim (an agent cannot make
/// a fabricated witness reproduce, nor inflate a claim past what the witness
/// structurally establishes), so matched attachments are non-load-bearing
/// corroboration and do NOT gate admission. This mirrors `exact_lane_eligible`'s
/// own guard #8, which waives the >=2-attachment requirement under
/// floor-sufficiency.
///
/// When the floor does NOT hold, attachments would be the load-bearing evidence
/// (the general / non-exact, e.g. Lean lane). Admitting there rests on
/// attachment provenance being a trustworthy HUMAN signal, and it is not yet:
/// actor registration is open self-enrollment, so a "registered non-agent
/// reviewer" can be a key an agent minted and self-registered under a
/// `reviewer:` id, then honestly signed with (an adversarial review confirmed
/// this self-enrollment bypass). Until the vouch binds to an owner/maintainer-
/// signed roster rooted in the frontier owner key, the non-floor lane REFUSES,
/// in the safe direction. (docs/VERIFICATION.md §7 guard #3.)
fn attachment_vouch_gate(floor_ok: bool, matched_len: usize) -> (bool, String) {
    if floor_ok {
        if matched_len == 0 {
            (true, String::new())
        } else {
            (
                true,
                format!(
                    "floor-sufficient: {matched_len} matched attachment(s) are non-load-bearing corroboration, not gating"
                ),
            )
        }
    } else {
        (
            false,
            "non-floor-sufficient admission would rest on attachment provenance, which is not yet owner-rooted (open actor self-enrollment is forgeable); lane refuses in the safe direction"
                .to_string(),
        )
    }
}

fn reproduce_finding_witness(
    proj: &vela_protocol::project::Project,
    frontier: &Path,
    finding_id: &str,
) -> (bool, String, Option<vela_verify::Witness>) {
    for art in &proj.artifacts {
        let is_json = art.media_type.as_deref() == Some("application/json");
        if !(is_json && art.metadata.contains_key("verifier")) {
            continue;
        }
        if !art.target_findings.iter().any(|t| t == finding_id) {
            continue;
        }
        let content = match (art.storage_mode.as_str(), &art.locator) {
            ("local_blob" | "local_file", Some(loc)) => {
                match std::fs::read_to_string(frontier.join(loc.as_str())) {
                    Ok(c) => c,
                    Err(e) => return (false, format!("witness unreadable: {e}"), None),
                }
            }
            _ => continue,
        };
        match parse_witness(&content) {
            Ok(w) => {
                let r = vela_verify::verify_witness(&w);
                return (r.ok, r.message, Some(w));
            }
            Err(e) => return (false, format!("witness parse failed: {e}"), None),
        }
    }
    (
        false,
        "no local frozen-verifier witness artifact targets this finding".to_string(),
        None,
    )
}

fn verified_local_artifact(
    frontier: &Path,
    artifact: &vela_protocol::bundle::Artifact,
) -> Result<PathBuf, String> {
    verified_frontier_file(
        frontier,
        &artifact.id,
        &artifact.storage_mode,
        artifact.locator.as_deref(),
        &artifact.content_hash,
    )
}

fn verified_frontier_file(
    frontier: &Path,
    artifact_id: &str,
    storage_mode: &str,
    locator: Option<&str>,
    expected_root: &str,
) -> Result<PathBuf, String> {
    if !matches!(storage_mode, "local_blob" | "local_file") {
        return Err(format!(
            "proposal verifier artifact {} is not locally reproducible",
            artifact_id
        ));
    }
    let locator = locator
        .ok_or_else(|| format!("proposal verifier artifact {artifact_id} has no locator"))?;
    let relative = Path::new(locator);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "proposal verifier artifact {} locator must remain frontier-relative",
            artifact_id
        ));
    }
    let frontier_root = std::fs::canonicalize(frontier)
        .map_err(|error| format!("resolve frontier root: {error}"))?;
    let file = frontier.join(relative);
    let metadata = std::fs::symlink_metadata(&file).map_err(|error| {
        format!(
            "inspect proposal verifier artifact {}: {error}",
            artifact_id
        )
    })?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(format!(
            "proposal verifier artifact {} must be a regular non-symlink file",
            artifact_id
        ));
    }
    let resolved = std::fs::canonicalize(&file).map_err(|error| {
        format!(
            "resolve proposal verifier artifact {}: {error}",
            artifact_id
        )
    })?;
    if !resolved.starts_with(&frontier_root) {
        return Err(format!(
            "proposal verifier artifact {} resolves outside the frontier",
            artifact_id
        ));
    }
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(&resolved)
        .map_err(|error| format!("read proposal verifier artifact {}: {error}", artifact_id))?;
    let observed = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    if observed != expected_root {
        return Err(format!(
            "proposal verifier artifact {} content root does not match retained bytes",
            artifact_id
        ));
    }
    Ok(resolved)
}

fn reproduction_result_path(frontier: &Path, file: &Path, proposal_scoped: bool) -> String {
    if !proposal_scoped {
        return file.display().to_string();
    }
    let root = std::fs::canonicalize(frontier).unwrap_or_else(|_| frontier.to_path_buf());
    let resolved = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    resolved.strip_prefix(&root).map_or_else(
        |_| {
            file.file_name().map_or_else(
                || "artifact".to_string(),
                |name| name.to_string_lossy().into(),
            )
        },
        |relative| relative.display().to_string(),
    )
}

pub(crate) fn proposal_reproduction_files(
    path: &Path,
    proposal_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let project =
        repo::load_from_path(path).map_err(|error| format!("load proposal frontier: {error}"))?;
    let proposal = project
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("proposal {proposal_id} does not exist"))?;
    if proposal.status != "pending_review" {
        return Err(format!(
            "proposal {proposal_id} is {}, not pending_review",
            proposal.status
        ));
    }
    if proposal.kind != "finding.add" {
        return Err(format!(
            "proposal-scoped reproduction requires finding.add, got {}",
            proposal.kind
        ));
    }
    let finding_id = proposal.target.id.as_str();
    let retained = project
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.media_type.as_deref() == Some("application/json")
                && artifact.metadata.contains_key("verifier")
                && artifact
                    .target_findings
                    .iter()
                    .any(|target| target == finding_id)
        })
        .collect::<Vec<_>>();
    if !retained.is_empty() {
        return retained
            .into_iter()
            .map(|artifact| {
                let file = verified_local_artifact(path, artifact)?;
                let raw = std::fs::read_to_string(&file).map_err(|error| {
                    format!("read proposal verifier artifact {}: {error}", artifact.id)
                })?;
                parse_witness(&raw).map_err(|error| {
                    format!(
                        "proposal verifier artifact {} is not a frozen witness: {error}",
                        artifact.id
                    )
                })?;
                Ok(file)
            })
            .collect::<Result<Vec<_>, _>>();
    }

    let submission = proposal
        .payload
        .get("vela_submission")
        .and_then(Value::as_object)
        .ok_or_else(|| "pending proposal has no Receipt binding".to_string())?;
    let receipt_path = submission
        .get("receipt_path")
        .and_then(Value::as_str)
        .ok_or_else(|| "proposal Receipt path is unavailable".to_string())?;
    let expected_root = submission
        .get("receipt_root")
        .and_then(Value::as_str)
        .ok_or_else(|| "proposal Receipt root is unavailable".to_string())?;
    let receipt_file = verified_frontier_relative_file(path, "proposal Receipt", receipt_path)?;
    let receipt_bytes = std::fs::read(&receipt_file)
        .map_err(|error| format!("read proposal Receipt {receipt_path}: {error}"))?;
    let receipt = vela_protocol::receipt_v1::ReceiptV1::parse(&receipt_bytes)
        .map_err(|error| format!("parse proposal Receipt: {error}"))?;
    let observed_root = receipt
        .canonical_root()
        .map_err(|error| format!("root proposal Receipt: {error}"))?;
    if observed_root != expected_root {
        return Err("proposal Receipt root does not match its retained bytes".to_string());
    }

    receipt
        .as_value()
        .get("artifacts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|artifact| {
            artifact
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.contains("witness"))
                || artifact
                    .get("path")
                    .and_then(Value::as_str)
                    .is_some_and(|path| path.ends_with(".witness.json"))
        })
        .filter_map(|artifact| artifact.get("sha256").and_then(Value::as_str))
        .map(|digest| {
            use sha2::{Digest, Sha256};
            let relative = format!("records/artifacts/sha256/{digest}");
            let file = verified_frontier_relative_file(path, "proposal artifact", &relative)?;
            let bytes = std::fs::read(&file)
                .map_err(|error| format!("read proposal artifact {}: {error}", file.display()))?;
            let observed = hex::encode(Sha256::digest(&bytes));
            if observed != digest {
                return Err("proposal artifact digest does not match retained bytes".to_string());
            }
            Ok(file)
        })
        .collect::<Result<Vec<_>, String>>()
}

fn verified_frontier_relative_file(
    frontier: &Path,
    label: &str,
    locator: &str,
) -> Result<PathBuf, String> {
    let relative = Path::new(locator);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!("{label} path must remain frontier-relative"));
    }
    let frontier_root = std::fs::canonicalize(frontier)
        .map_err(|error| format!("resolve frontier root: {error}"))?;
    let file = frontier.join(relative);
    let metadata =
        std::fs::symlink_metadata(&file).map_err(|error| format!("inspect {label}: {error}"))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(format!("{label} must be a regular non-symlink file"));
    }
    let resolved =
        std::fs::canonicalize(&file).map_err(|error| format!("resolve {label}: {error}"))?;
    if !resolved.starts_with(&frontier_root) {
        return Err(format!("{label} resolves outside the frontier"));
    }
    Ok(resolved)
}

pub(crate) fn cmd_reproduce(path: &Path, proposal_id: Option<&str>, json_output: bool) {
    crate::ui::set_mode("reproduce", json_output);
    let mut scope = if path.is_file() {
        "standalone_artifact"
    } else {
        "accepted_frontier"
    };
    if !json_output {
        crate::ui::header("REPRODUCE", &path.display().to_string(), None);
    }
    let mut files = if let Some(proposal_id) = proposal_id {
        scope = "pending_proposal";
        proposal_reproduction_files(path, proposal_id).unwrap_or_else(|error| fail_return(&error))
    } else {
        collect_witness_files(path)
    };
    // Also re-verify content-addressed witness blobs landed as verifier-tagged
    // artifacts under `.vela/artifact-blobs/`, bound to their findings rather
    // than stored as loose files. Cover any blob not already present as a loose
    // witness (deduped by content hash).
    if proposal_id.is_none()
        && let Ok(source) = repo::detect(path)
        && let Ok(proj) = repo::load(&source)
    {
        use sha2::{Digest, Sha256};
        let loose: std::collections::HashSet<String> = files
            .iter()
            .filter_map(|f| std::fs::read(f).ok())
            .map(|b| format!("sha256:{}", hex::encode(Sha256::digest(&b))))
            .collect();
        for art in &proj.artifacts {
            if art.media_type.as_deref() != Some("application/json")
                || !art.metadata.contains_key("verifier")
                || loose.contains(&art.content_hash)
            {
                continue;
            }
            if let ("local_blob" | "local_file", Some(loc)) =
                (art.storage_mode.as_str(), &art.locator)
            {
                let blob = path.join(loc);
                if blob.exists() {
                    files.push(blob);
                }
            }
        }
    }
    if files.is_empty() {
        if let Some(proposal_id) = proposal_id {
            fail(&format!(
                "proposal {proposal_id} has no frontier-local frozen witness to reproduce; inspect its retained artifacts and verifier evidence, or use the producer's exact replay bundle"
            ));
        }
        fail(&format!(
            "no witnesses found at {} (expected a `*.witness.json` file, or a directory containing them / a `witnesses/` subdir)",
            path.display()
        ));
    }
    let spinner = (!json_output).then(|| {
        crate::cli::progress::Spinner::start(&format!(
            "re-verifying {} witness(es) with the frozen verifiers",
            files.len()
        ))
    });
    let mut results: Vec<Value> = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    for file in &files {
        let result_path = reproduction_result_path(path, file, proposal_id.is_some());
        let raw = match std::fs::read_to_string(file) {
            Ok(r) => r,
            Err(e) => {
                failed += 1;
                if !json_output {
                    println!("  FAIL  {result_path}  ·  read error: {e}");
                }
                results.push(json!({"path": result_path, "ok": false, "message": format!("read error: {e}")}));
                continue;
            }
        };
        let witness = match parse_witness(&raw) {
            Ok(w) => w,
            Err(e) => {
                failed += 1;
                if !json_output {
                    println!("  FAIL  {result_path}  ·  parse error: {e}");
                }
                results.push(json!({"path": result_path, "ok": false, "message": format!("parse error: {e}")}));
                continue;
            }
        };
        let mut outcome = vela_verify::verify_witness(&witness);
        // Machine-checked novelty: a witness may declare `improves_on`
        // (a sibling witness path relative to its own directory). The
        // claim then verifies ONLY if it also strictly dominates the
        // referenced witness — dominance is arithmetic, not opinion.
        if outcome.ok
            && let Ok(value) = serde_json::from_str::<Value>(&raw)
            && let Some(prior_rel) = value.get("improves_on").and_then(Value::as_str)
        {
            let prior_path = file
                .parent()
                .map(|d| d.join(prior_rel))
                .unwrap_or_else(|| std::path::PathBuf::from(prior_rel));
            match std::fs::read_to_string(&prior_path)
                .map_err(|e| format!("improves_on read {}: {e}", prior_path.display()))
                .and_then(|p| parse_witness(&p))
                .and_then(|prior| vela_verify::dominates(&witness, &prior))
            {
                Ok(true) => {
                    outcome.message =
                        format!("{} · strictly improves on {prior_rel}", outcome.message);
                }
                Ok(false) => {
                    outcome = vela_verify::VerifyResult::fail(format!(
                        "claims improves_on {prior_rel} but does NOT strictly dominate it"
                    ));
                }
                Err(e) => {
                    outcome =
                        vela_verify::VerifyResult::fail(format!("improves_on check failed: {e}"));
                }
            }
        }
        if outcome.ok {
            passed += 1;
        } else {
            failed += 1;
        }
        if !json_output {
            let status = if outcome.ok { "ok  " } else { "FAIL" };
            println!(
                "  {status}  {} [{}]  ·  {}",
                result_path,
                witness.kind(),
                outcome.message
            );
        }
        results.push(json!({
            "path": result_path,
            "kind": witness.kind(),
            "ok": outcome.ok,
            "message": outcome.message,
        }));
    }
    if let Some(s) = spinner {
        s.finish(&format!("{passed} verified, {failed} failed"));
    }
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "command": "reproduce",
                "scope": scope,
                "proposal_id": proposal_id,
                "authority_effect": "none",
                "witnesses": files.len(),
                "passed": passed,
                "failed": failed,
                "results": results,
            }))
            .expect("serialize reproduce response")
        );
    } else {
        println!();
        println!("  scope: {scope}");
        if let Some(proposal_id) = proposal_id {
            println!("  proposal: {proposal_id} (pending; acceptance unchanged)");
        }
        if failed == 0 {
            println!(
                "  reproduce: ok ({passed}/{}) — every witness re-verified from scratch by the frozen verifiers.",
                files.len()
            );
        } else {
            println!(
                "  reproduce: FAIL ({failed}/{} did not re-verify). Investigate before trusting.",
                files.len()
            );
        }
    }
    if failed > 0 {
        std::process::exit(1);
    }
}

pub(crate) fn cmd_evidence_ci(frontier: &Path, json: bool) {
    let project = repo::load_from_path(frontier)
        .unwrap_or_else(|error| fail_return(&format!("evidence-ci failed: {error}")));
    let snapshot = active_policy_pair_snapshot(frontier);
    let policy = vela_protocol::proposals::policy_accept::assess_policy_readiness(
        &project,
        snapshot.as_ref().map_err(String::as_str),
        &chrono::Utc::now().to_rfc3339(),
    );
    if policy.permit_readiness()
        == vela_protocol::proposals::policy_accept::PermitReadiness::Blocked
    {
        if json {
            print_json(&json!({
                "ok": false,
                "command": "evidence-ci",
                "policy": {
                    "state": policy.state().as_str(),
                    "permit_readiness": policy.permit_readiness().as_str(),
                    "reason_codes": policy.reason_codes(),
                    "error": policy.detail(),
                },
            }));
            std::process::exit(1);
        }
        fail_return::<()>(&format!(
            "evidence-ci failed: active policy {}",
            policy.detail().unwrap_or("readiness assessment is blocked")
        ));
    }
    let report = evidence_ci::run_frontier(frontier)
        .unwrap_or_else(|e| fail_return(&format!("evidence-ci failed: {e}")));
    if json {
        print_json(&report);
        return;
    }
    let status = if report.ok {
        style::ok("evidence-ci")
    } else {
        style::lost("evidence-ci")
    };
    println!(
        "{} {} · {} checks, {} warning(s), {} release-blocking failure(s)",
        status,
        report.frontier_id,
        report.summary.total,
        report.summary.warnings,
        report.summary.release_blocking_failed
    );
    for check in report
        .checks
        .iter()
        .filter(|check| check.status != evidence_ci::EvidenceCiStatus::Passed)
        .take(40)
    {
        println!(
            "  {} {} {}: {}",
            match check.status {
                evidence_ci::EvidenceCiStatus::Passed => style::ok("pass"),
                evidence_ci::EvidenceCiStatus::Warning => style::warn("warn"),
                evidence_ci::EvidenceCiStatus::Failed => style::lost("fail"),
            },
            check.id,
            check.target_id,
            check.message
        );
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;
    use sha2::{Digest, Sha256};

    // The exact-lane vouch gate. An adversarial review showed the prior vouch
    // (a "registered non-agent reviewer" signing a verifier_attachment.added
    // event, or accepting a verifier.attach proposal) is forgeable: actor
    // registration is open self-enrollment, so an agent mints a key, registers
    // `reviewer:x`, and honestly signs. The fix scopes the vouch to where
    // attachments are load-bearing (the non-floor lane), and admits the exact
    // lane on the un-forgeable FLOOR alone.

    #[test]
    fn floor_sufficient_admits_without_any_vouch_even_with_attachments() {
        // The exact lane: the floor (fresh reproduce + claim_witness_faithful)
        // is the proof. Matched attachments are non-load-bearing corroboration
        // and must NOT block admission, regardless of who signed them. This is
        // what lets the foundry compound without a human, soundly.
        let (ok, reason) = attachment_vouch_gate(true, 0);
        assert!(ok, "floor-sufficient with no attachments admits");
        assert!(reason.is_empty());

        let (ok, reason) = attachment_vouch_gate(true, 3);
        assert!(
            ok,
            "floor-sufficient admits even with attachments present (they do not gate)"
        );
        assert!(
            reason.contains("non-load-bearing"),
            "the reason is honest that attachments are not gating: {reason}"
        );
    }

    #[test]
    fn non_floor_sufficient_refuses_until_vouch_is_owner_rooted() {
        // The non-exact / Lean lane, where attachments WOULD be the evidence.
        // Because the only vouch root today is open self-enrollment (forgeable),
        // the lane must refuse in the safe direction rather than admit on a
        // forgeable vouch. This keeps the de-human-gate firing ONLY on the
        // un-forgeable floor.
        for matched_len in [0usize, 1, 5] {
            let (ok, reason) = attachment_vouch_gate(false, matched_len);
            assert!(
                !ok,
                "non-floor-sufficient must refuse (matched_len={matched_len})"
            );
            assert!(
                reason.contains("owner-rooted") || reason.contains("refuses"),
                "the refusal names the missing owner-rooted vouch: {reason}"
            );
        }
    }

    #[test]
    fn proposal_reproduction_reads_only_rooted_frontier_files() {
        let frontier = tempfile::tempdir().unwrap();
        std::fs::create_dir(frontier.path().join("records")).unwrap();
        let bytes = br#"{"schema":"fixture"}"#;
        std::fs::write(frontier.path().join("records/witness.json"), bytes).unwrap();
        let root = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
        let resolved = verified_frontier_file(
            frontier.path(),
            "va_fixture",
            "local_file",
            Some("records/witness.json"),
            &root,
        )
        .unwrap();
        assert!(resolved.starts_with(std::fs::canonicalize(frontier.path()).unwrap()));
        assert_eq!(
            reproduction_result_path(frontier.path(), &resolved, true),
            "records/witness.json"
        );

        let traversal = verified_frontier_file(
            frontier.path(),
            "va_fixture",
            "local_file",
            Some("../secret.json"),
            &root,
        )
        .unwrap_err();
        assert!(traversal.contains("frontier-relative"));

        let tampered = verified_frontier_file(
            frontier.path(),
            "va_fixture",
            "local_file",
            Some("records/witness.json"),
            &format!("sha256:{}", "0".repeat(64)),
        )
        .unwrap_err();
        assert!(tampered.contains("content root"));
    }
}
