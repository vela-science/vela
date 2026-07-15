use crate::cli::{collect_witness_files, fail, fail_return, parse_witness, print_json};
use crate::cli_commands::*;
use serde_json::{Value, json};
use std::path::Path;
use vela_protocol::cli_style as style;
use vela_protocol::evidence_ci;
use vela_protocol::repo;

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
    match vela_protocol::acceptance_policy::load_active_policy(frontier) {
        Ok(Some(vp)) => {
            let now = chrono::Utc::now().to_rfc3339();
            vela_protocol::acceptance_policy::resolve_policy_authority(&proj, &vp, &now)
                .map_err(|error| format!("active policy authority: {error}"))?;
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
            let permitted = format!("{:?}", decision.outcome) == "Permit";
            would_permit = would_permit && permitted;
        }
        Ok(None) => {}
        Err(e) => return Err(format!("active policy: {e}")),
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

// ---- the foundry: discovery and verifier tools ----

/// Foundry commands produce discovery artifacts for the shared
/// `next -> work -> land` loop. Frontier mutation and policy routing stay in
/// Receipt v1 landing.
pub(crate) fn cmd_foundry(action: FoundryAction) {
    match action {
        FoundryAction::Campaign { action } => crate::cli_campaign::cmd_campaign(action),
        FoundryAction::Lean { action } => crate::cli_lean::cmd_lean(action),
        FoundryAction::Attempt { action } => crate::cli_lean::cmd_attempt(action),
        FoundryAction::Transfer { action } => crate::cli_lean::cmd_transfer(action),
        FoundryAction::Experiment { action } => crate::cli_experiment::cmd_experiment(action),
        FoundryAction::Targets {
            catalog,
            records,
            attackable_only,
            erdos_bounds,
            json,
        } => cmd_foundry_targets(
            &catalog,
            &records,
            attackable_only,
            erdos_bounds.as_deref(),
            json,
        ),
        FoundryAction::Ablate {
            frontier,
            kind,
            records,
            n,
            h,
            budget,
            seeds,
            json,
        } => cmd_foundry_ablate(
            &frontier,
            &kind,
            records.as_deref(),
            n,
            h,
            budget,
            seeds,
            json,
        ),
        FoundryAction::LeanTargets {
            lean_dir,
            subdir,
            all,
            limit,
            json,
        } => cmd_foundry_lean_targets(&lean_dir, &subdir, all, limit, json),
        FoundryAction::LeanAblate {
            frontier,
            lemmas,
            json,
        } => cmd_foundry_lean_ablate(&frontier, lemmas.as_deref(), json),
        FoundryAction::ErdosBounds { input, out, json } => {
            cmd_foundry_erdos_bounds(&input, &out, json)
        }
    }
}

/// Project the typed current-best bounds from the staged erdos-deep source into
/// a `vela.frontier-bounds.v1` sidecar. Reads the SAME source as the pure Erdős
/// adapter, runs it through the adapter's typed-bound projection, and writes a
/// new `bounds.json`. Additive: it never reads or writes any accepted finding
/// or the frontier canonical root, so `vela reproduce` is unaffected. Every
/// bound is unattested (`accepted: false`). Deterministic (sorted output).
fn cmd_foundry_erdos_bounds(input: &Path, out: &Path, json_out: bool) {
    let records = crate::atlas_adapters::read_erdos_deep(input, "erdos-bounds")
        .unwrap_or_else(|e| fail_return(&e));
    let doc = crate::atlas_adapters::erdos_deep_bounds(&records);
    let body = serde_json::to_string_pretty(&doc).unwrap() + "\n";
    std::fs::write(out, &body)
        .unwrap_or_else(|e| fail_return(&format!("write {}: {e}", out.display())));
    let attackable = doc.bounds.iter().filter(|b| b.value.is_some()).count();
    if json_out {
        print_json(&json!({
            "command": "foundry.erdos-bounds",
            "input": input.display().to_string(),
            "out": out.display().to_string(),
            "schema": doc.schema,
            "bounds": doc.bounds.len(),
            "parsed_value": attackable,
            "all_unattested": doc.bounds.iter().all(|b| !b.accepted),
        }));
    } else {
        eprintln!(
            "wrote {} ({} typed bounds, {} with a parsed value; all unattested)",
            out.display(),
            doc.bounds.len(),
            attackable
        );
    }
}

/// The decisive lemma-inheritance measurement (the memo's "Compounding B"):
/// do accepted Lean lemmas widen the closable boundary? Treatment counts the
/// open targets that are one-premise-away WITH the inherited lemmas present;
/// control demotes those lemmas to Open. Δ>0 = inherited verified state makes
/// the next proof reachable (the formal analogue of skip-known-work). Unlike a
/// search ablation this measures UNLOCK STRUCTURE, not solver yield.
fn cmd_foundry_lean_ablate(frontier: &Path, lemmas: Option<&str>, json_out: bool) {
    use std::collections::BTreeSet;
    use vela_protocol::boundary::Boundary;
    let source = repo::detect(frontier).unwrap_or_else(|e| fail_return(&e));
    let proj = repo::load(&source).unwrap_or_else(|e| fail_return(&e));
    let inherited: BTreeSet<String> = match lemmas {
        Some(s) => s
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect(),
        None => proj
            .findings
            .iter()
            .filter(|f| f.assertion.assertion_type.to_lowercase().contains("lean"))
            .map(|f| f.id.clone())
            .collect(),
    };
    let treatment = Boundary::derive(&proj).one_premise_away.len();
    let control = Boundary::derive_with_demoted(&proj, &inherited)
        .one_premise_away
        .len();
    let delta = treatment as i64 - control as i64;
    let compounds = delta > 0;
    if json_out {
        print_json(&json!({
            "command": "foundry.lean-ablate",
            "frontier": frontier.display().to_string(),
            "inherited_lemmas": inherited.len(),
            "treatment": treatment,
            "control": control,
            "delta": delta,
            "inheritance_compounds": compounds,
            "note": "treatment = one-premise-away targets with inherited Lean lemmas present; \
                     control demotes them to Open. Δ>0 = accepted lemmas widen the closable \
                     boundary (Compounding B). Requires inter-problem premise edges (WS-C5) to be \
                     non-zero.",
        }));
    } else {
        println!(
            "{} inherited_lemmas={} treatment={} control={} Δ={} compounds={}",
            style::ok("foundry.lean-ablate"),
            inherited.len(),
            treatment,
            control,
            delta,
            compounds,
        );
        if treatment == 0 && control == 0 {
            println!(
                "  (0/0 — no Lean one-premise-away structure yet; the measurement needs \
                 inter-problem premise edges, i.e. WS-C5 math judgment)"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The prover-in-the-loop Lean lane (Program 1 of the Known->Unknown plan).
// Known proved lemmas compose into proofs of open theorems. The AI (proof-
// subagents) is the PRODUCER of a candidate Lean proof; the Lean kernel +
// `#print axioms` classification is the frozen VERIFIER (the analogue of `splr`
// producing a SAT assignment that `verify_diff_triangle` checks); a human key
// ACCEPTS. No model is ever in a trust path.
// ---------------------------------------------------------------------------

/// One open Lean obligation surfaced by `vela foundry lean-targets`.
#[derive(serde::Serialize)]
struct LeanTarget {
    module: String,
    /// Fully-qualified decl (`Namespace.decl`) for a later kernel/axiom audit.
    decl: String,
    namespace: String,
    category: String,
    /// `formalization_gap` (a non-research-open decl still carrying `sorry` — the
    /// tractable target) or `research_open` (the headline open problem, not
    /// expected subagent-closable).
    tractability: String,
}

/// Read-only worklist: scan a formal-conjectures corpus for OPEN Lean
/// obligations (decls carrying `sorry`), ranked by a heuristic tractability so
/// the prove loop attacks the formalization gaps first. A pinned Lean kernel
/// build plus axiom audit is the real arbiter; this command only orders the queue.
fn cmd_foundry_lean_targets(
    lean_dir: &Path,
    subdir: &str,
    all: bool,
    limit: usize,
    json_out: bool,
) {
    let root = lean_dir.join(subdir);
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect_lean_files(&root, &mut files);
    files.sort();

    let mut targets: Vec<LeanTarget> = Vec::new();
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        let module = f
            .strip_prefix(lean_dir)
            .unwrap_or(f)
            .to_string_lossy()
            .to_string();
        targets.extend(scan_lean_decls(&text, &module));
    }
    // Tractable gaps first, then by module for determinism.
    targets.sort_by(|a, b| {
        let rank = |t: &str| if t == "formalization_gap" { 0 } else { 1 };
        rank(&a.tractability)
            .cmp(&rank(&b.tractability))
            .then(a.module.cmp(&b.module))
            .then(a.decl.cmp(&b.decl))
    });
    let shown: Vec<&LeanTarget> = targets
        .iter()
        .filter(|t| all || t.tractability == "formalization_gap")
        .take(limit)
        .collect();

    if json_out {
        print_json(&json!({
            "command": "foundry.lean-targets",
            "lean_dir": lean_dir.display().to_string(),
            "subdir": subdir,
            "open_total": targets.len(),
            "shown": shown.len(),
            "note": "tractability is a heuristic; a pinned Lean kernel build plus axiom audit is the arbiter. \
                     research_open decls are the headline problems, not expected subagent-closable.",
            "targets": shown,
        }));
    } else {
        println!(
            "open Lean obligations in {} — {} ({} tractable gaps surfaced)",
            root.display(),
            targets.len(),
            shown
                .iter()
                .filter(|t| t.tractability == "formalization_gap")
                .count(),
        );
        for t in &shown {
            println!("  [{}] {}  ({})", t.tractability, t.decl, t.module);
        }
        if shown.is_empty() {
            println!(
                "  (no tractable gaps; pass --all to list the headline research-open problems)"
            );
        }
        println!(
            "\n  honest: tractability is heuristic; a pinned Lean kernel build plus \
             #print axioms audit is the arbiter. most research-open Erdős problems are not \
             subagent-closable."
        );
    }
}

/// Recursively collect `*.lean` files under `dir`.
fn collect_lean_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() {
            collect_lean_files(&p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("lean") {
            out.push(p);
        }
    }
}

/// Parse a Lean source for open obligations: each `theorem`/`lemma` whose block
/// (up to the next decl) still contains `sorry`. Captures the opened namespace,
/// the `@[category ...]` tag, and a tractability heuristic.
fn scan_lean_decls(text: &str, module: &str) -> Vec<LeanTarget> {
    let lines: Vec<&str> = text.lines().collect();
    // Decl positions + the namespace open just before them.
    let mut namespace = String::new();
    let mut decl_starts: Vec<(usize, String)> = Vec::new(); // (line idx, short decl)
    for (i, raw) in lines.iter().enumerate() {
        let l = raw.trim_start();
        if let Some(rest) = l.strip_prefix("namespace ") {
            namespace = rest.split_whitespace().next().unwrap_or("").to_string();
        }
        for kw in ["theorem ", "lemma "] {
            if let Some(rest) = l.strip_prefix(kw) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.' || *c == '\'')
                    .collect();
                if !name.is_empty() {
                    decl_starts.push((i, name));
                }
                break;
            }
        }
    }
    let mut out = Vec::new();
    for (idx, (start, short)) in decl_starts.iter().enumerate() {
        let end = decl_starts
            .get(idx + 1)
            .map(|(j, _)| *j)
            .unwrap_or(lines.len());
        let block = lines[*start..end].join("\n");
        if !block.contains("sorry") {
            continue; // already proved -> not an open obligation
        }
        // The `@[category ...]` tag is on the lines just above the decl.
        let cat_start = start.saturating_sub(4);
        let header = lines[cat_start..*start].join(" ");
        let category = if let Some(i) = header.find("category ") {
            header[i + "category ".len()..]
                .split([',', ']'])
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        } else {
            String::new()
        };
        let research_open = header.contains("research open");
        let tractability = if research_open {
            "research_open"
        } else {
            "formalization_gap"
        }
        .to_string();
        let fq = if namespace.is_empty() {
            short.clone()
        } else {
            format!("{namespace}.{short}")
        };
        out.push(LeanTarget {
            module: module.to_string(),
            decl: fq,
            namespace: namespace.clone(),
            category,
            tractability,
        });
    }
    out
}

/// The continuous-ablation heartbeat: does inherited frontier state make the
/// next solver go farther per unit compute? The honest skip-known-work form
/// (the H1 result): at a FIXED budget, inheriting the frontier's `known` solved
/// targets lets the producer concentrate the whole budget on the boundary
/// (TREATMENT); a producer with no inherited state must spread the same budget
/// across the `known + 1` targets it might need to rediscover (CONTROL, the
/// boundary gets `budget / (known + 1)`). Over `seeds` deterministic runs, the
/// difference in boundary-success rate is the inheritance effect. Exits 1 if
/// treatment does not beat control (the plan's hard gate).
#[allow(clippy::too_many_arguments)]
fn cmd_foundry_ablate(
    frontier: &Path,
    kind: &str,
    records: Option<&Path>,
    boundary: usize,
    h: usize,
    budget: u64,
    seeds: u64,
    json_out: bool,
) {
    // The inherited state: how many solved targets of this kind are already
    // banked (the depth a no-inheritance producer would have to rediscover).
    // Either from a per-family records catalog (accepted, reproduce-backed
    // bounds — runs without a key-custody accept ceremony) or, by default, from
    // the frontier's accepted findings (matched by the kind keyword, so it works
    // for kinds with no `{0,1}^n` ambient dimension like golomb/costas too).
    let known = if let Some(records_path) = records {
        let raw = std::fs::read_to_string(records_path)
            .unwrap_or_else(|e| fail_return(&format!("read {}: {e}", records_path.display())));
        let v: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|e| fail_return(&format!("parse {}: {e}", records_path.display())));
        // Accept either the `records/<family>.json` schema (`bounds[].accepted`)
        // or the producer `bounds.json` (`bounds[].accepted`).
        v.get("bounds")
            .and_then(|b| b.as_array())
            .map(|arr| {
                arr.iter()
                    .filter(|e| e.get("accepted").and_then(|a| a.as_bool()).unwrap_or(false))
                    .count()
            })
            .unwrap_or(0)
    } else {
        let source = repo::detect(frontier).unwrap_or_else(|e| fail_return(&e));
        let proj = repo::load(&source).unwrap_or_else(|e| fail_return(&e));
        proj.findings
            .iter()
            .filter(|f| f.assertion.text.to_lowercase().contains(kind))
            .count()
    };
    let range = (known as u64) + 1; // the targets a no-inheritance producer covers
    let control_budget = (budget / range).max(1);

    let target = crate::campaign::Target {
        kind: kind.to_string(),
        n: boundary,
        h,
        d: 0,
        w: 0,
        k: 0,
        t: 0,
    };

    // The H1 metric is the SCORE (witness size / frontier order), not
    // found/not-found: a witness usually exists, the question is how BIG a one
    // each arm reaches with its budget. Mean score over `seeds` deterministic
    // runs; treatment concentrates the full budget, control gets the spread.
    let mut t_total = 0u64;
    let mut c_total = 0u64;
    for seed in 1..=seeds {
        let score_of = |restarts: u64| -> u64 {
            match crate::campaign::search_target(&target, restarts, seed) {
                Ok(Some(found)) => found.score as u64,
                _ => 0,
            }
        };
        t_total += score_of(budget);
        c_total += score_of(control_budget);
    }
    let t_mean = t_total as f64 / seeds as f64;
    let c_mean = c_total as f64 / seeds as f64;
    let delta = t_mean - c_mean;
    let inheritance_compounds = t_mean > c_mean;

    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "kind": kind,
                "boundary": boundary,
                "inherited_solved_targets": known,
                "budget": budget,
                "control_budget_per_boundary": control_budget,
                "seeds": seeds,
                "treatment_mean_score": t_mean,
                "control_mean_score": c_mean,
                "delta": delta,
                "inheritance_compounds": inheritance_compounds,
            }))
            .unwrap()
        );
    } else {
        println!("continuous ablation — {kind} boundary n={boundary}:");
        println!("  inherited solved targets (skip-known-work depth): {known}");
        println!("  fixed budget per arm: {budget} restarts");
        println!("  TREATMENT (inherit -> full {budget} on boundary): mean score {t_mean:.2}");
        println!(
            "  CONTROL   (no inherit -> {control_budget}/boundary):        mean score {c_mean:.2}"
        );
        if known == 0 {
            println!(
                "  => no inherited state for '{kind}' on this frontier (N/A — nothing to inherit)"
            );
        } else {
            println!(
                "  => inheritance compounds: {} (Δ {:+.2} frontier orders)",
                if inheritance_compounds { "YES" } else { "NO" },
                delta
            );
        }
    }
    // Informational by default (a measurement, not a self-gate): exit 0 always.
    // Campaign evaluation or CI gates by reading `inheritance_compounds` in the JSON.
    // Only a kind that is BOTH a real compute-lever AND carries inherited state
    // is expected to compound — sidon is greedy-saturated (H1), golomb is the
    // lever; the reading reflects that honestly per (kind, frontier).
}

/// The `vela campaign` engine kinds — the verifier families the foundry can
/// actually attack (every one has a `search_*` in `campaign.rs`).
const FOUNDRY_ENGINE_KINDS: &[&str] = &[
    "gf2_sidon",
    "union_free",
    "rook_directions",
    "cap",
    "constant_weight",
    "covering",
    "sidon",
    "bh",
    "golomb",
    "costas",
    "diff_triangle",
];

/// The current Vela-accepted extent from a `bounds.json`-shaped records file
/// (count of accepted records + the deepest `n` reached and its bound), or None
/// if the file is absent/empty. The honest "what Vela already holds" against
/// which a value-to-beat reads as a gap.
fn read_records_best(path: &Path) -> Option<Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    let doc: Value = serde_json::from_str(&raw).ok()?;
    let bounds = doc.get("bounds")?.as_array()?;
    let mut count = 0i64;
    let mut max_n = -1i64;
    let mut bound_at_max = 0i64;
    for b in bounds {
        if !b.get("accepted").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        count += 1;
        let n = b.get("n").and_then(|x| x.as_i64()).unwrap_or(0);
        if n > max_n {
            max_n = n;
            bound_at_max = b
                .get("best_lower_bound")
                .and_then(|x| x.as_i64())
                .unwrap_or(0);
        }
    }
    (count > 0).then(
        || json!({ "accepted_records": count, "max_n": max_n, "bound_at_max_n": bound_at_max }),
    )
}

/// `vela foundry targets`: the foundry's substrate-native work-list. Read the
/// target catalog, cross-reference the live per-family records, and print the
/// attackable portfolio with each value-to-beat (and the current accepted best
/// where Vela holds records). This replaces the web/script JSON as the foundry's
/// portfolio source; campaign search selects a cell from it.
fn cmd_foundry_targets(
    catalog: &Path,
    records: &Path,
    attackable_only: bool,
    erdos_bounds: Option<&Path>,
    json_out: bool,
) {
    let raw = std::fs::read_to_string(catalog)
        .unwrap_or_else(|e| fail_return(&format!("read {}: {e}", catalog.display())));
    let doc: Value = serde_json::from_str(&raw)
        .unwrap_or_else(|e| fail_return(&format!("parse {}: {e}", catalog.display())));
    let problems = doc
        .get("problems")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Live accepted extent per family from the standardized
    // `frontiers/<kind>/records.json` catalogs. Path relative to `--records`.
    let records_path = |kind: &str| -> std::path::PathBuf {
        if kind == "sidon" {
            records.join("sidon/records.json")
        } else {
            records.join(format!("{kind}/records.json"))
        }
    };

    let mut rows: Vec<Value> = Vec::new();
    for p in &problems {
        let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let kind = p
            .get("verifier_kind")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let attackable = FOUNDRY_ENGINE_KINDS.contains(&kind);
        if attackable_only && !attackable {
            continue;
        }
        let status = p.get("status").and_then(|v| v.as_str()).unwrap_or("open");
        let inc = p.get("incumbent");
        let value = inc
            .and_then(|i| i.get("value"))
            .filter(|v| !v.is_null())
            .cloned();
        let direction = inc
            .and_then(|i| i.get("direction"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let basis = inc
            .and_then(|i| i.get("basis"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let rpath = records_path(kind);
        let accepted_best = read_records_best(&rpath);
        let records_source = accepted_best.as_ref().map(|_| rpath.display().to_string());
        rows.push(json!({
            "id": id,
            "domain": p.get("domain"),
            "level": p.get("level"),
            "verifier_kind": kind,
            "attackable": attackable,
            "params": p.get("params"),
            "value_to_beat": value,
            "direction": direction,
            "basis": basis,
            "status": status,
            "source": p.get("source"),
            "accepted_best": accepted_best,
            "records_source": records_source,
        }));
    }

    // Optionally fold in the typed Erdős current-best bounds (the value-to-beat
    // the erdos-deep adapter now emits into a `vela.frontier-bounds.v1` sidecar).
    // This is the consumer that READS the typed bound — it surfaces each Erdős
    // problem's value-to-beat in the same portfolio as the catalog incumbents,
    // so the foundry / attack ranking sees it. Unattested bounds are honestly
    // labeled (status "bound-unattested"); non-engine kind (no campaign attacks
    // an arbitrary Erdős problem yet) keeps them ranked after the engine cells.
    if let Some(bp) = erdos_bounds {
        match std::fs::read_to_string(bp) {
            Ok(braw) => {
                match serde_json::from_str::<vela_protocol::frontier_bound::FrontierBoundsDoc>(
                    &braw,
                ) {
                    Ok(doc) => {
                        for b in &doc.bounds {
                            rows.push(json!({
                            "id": b.problem,
                            "domain": "erdos",
                            "verifier_kind": "",
                            "attackable": false,
                            "value_to_beat": b.value,
                            "direction": b.direction.as_str(),
                            "basis": b.source_text,
                            "status": if b.accepted { "bound-attested" } else { "bound-unattested" },
                            "source": bp.display().to_string(),
                            "accepted_best": Value::Null,
                            "records_source": Value::Null,
                        }));
                        }
                    }
                    Err(e) => fail_return(&format!("parse {}: {e}", bp.display())),
                }
            }
            Err(e) => fail_return(&format!("read {}: {e}", bp.display())),
        }
    }

    // Sort: attackable+open first; non-engine kinds and showcases last.
    rows.sort_by(|a, b| {
        let key = |r: &Value| -> (u8, u8, String) {
            let att = if r["attackable"].as_bool().unwrap_or(false) {
                0
            } else {
                1
            };
            let st = match r["status"].as_str().unwrap_or("") {
                "open" => 0,
                "verified_showcase" => 2,
                _ => 1,
            };
            (att, st, r["id"].as_str().unwrap_or("").to_string())
        };
        key(a).cmp(&key(b))
    });

    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "catalog": catalog.display().to_string(),
                "targets": rows.len(),
                "portfolio": rows,
            }))
            .unwrap()
        );
        return;
    }

    println!(
        "foundry targets — {} cells ({}):",
        rows.len(),
        catalog.display()
    );
    for r in &rows {
        let id = r["id"].as_str().unwrap_or("");
        let kind = r["verifier_kind"].as_str().unwrap_or("");
        let dir = r["direction"].as_str().unwrap_or("");
        let vtb = match &r["value_to_beat"] {
            Value::Null => "per-parameter".to_string(),
            v => v.to_string(),
        };
        let best = match &r["accepted_best"] {
            Value::Object(m) => format!(
                "{} records (n<={})",
                m.get("accepted_records")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                m.get("max_n").and_then(|v| v.as_i64()).unwrap_or(0)
            ),
            _ => "none".to_string(),
        };
        let status = r["status"].as_str().unwrap_or("");
        let flag = if r["attackable"].as_bool().unwrap_or(false) {
            ""
        } else {
            " (no engine kind)"
        };
        println!("  {id:<24} {kind:<16} beat {vtb} ({dir})  accepted {best}  [{status}]{flag}");
    }
    println!("\nattack one with: vela foundry campaign search <verifier_kind> --n <param> --json");
}

/// Re-run the frozen verifier over the finding's witness artifact (the
/// reproduce-binding the exact lane computes itself, never trusting a field).
/// Returns (ok, human-detail, the parsed witness).
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

pub(crate) fn cmd_reproduce(path: &Path, json_output: bool) {
    crate::ui::set_mode("reproduce", json_output);
    if !json_output {
        crate::ui::header("REPRODUCE", &path.display().to_string(), None);
    }
    let mut files = collect_witness_files(path);
    // Also re-verify content-addressed witness blobs landed as verifier-tagged
    // artifacts under `.vela/artifact-blobs/`, bound to their findings rather
    // than stored as loose files. Cover any blob not already present as a loose
    // witness (deduped by content hash).
    if let Ok(source) = repo::detect(path)
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
        let raw = match std::fs::read_to_string(file) {
            Ok(r) => r,
            Err(e) => {
                failed += 1;
                if !json_output {
                    println!("  FAIL  {}  ·  read error: {e}", file.display());
                }
                results.push(json!({"path": file.display().to_string(), "ok": false, "message": format!("read error: {e}")}));
                continue;
            }
        };
        let witness = match parse_witness(&raw) {
            Ok(w) => w,
            Err(e) => {
                failed += 1;
                if !json_output {
                    println!("  FAIL  {}  ·  parse error: {e}", file.display());
                }
                results.push(json!({"path": file.display().to_string(), "ok": false, "message": format!("parse error: {e}")}));
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
                file.display(),
                witness.kind(),
                outcome.message
            );
        }
        results.push(json!({
            "path": file.display().to_string(),
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
                "witnesses": files.len(),
                "passed": passed,
                "failed": failed,
                "results": results,
            }))
            .expect("serialize reproduce response")
        );
    } else {
        println!();
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
mod foundry_targets_tests {
    use super::*;

    #[test]
    fn read_records_best_reports_deepest_accepted() {
        let dir = std::env::temp_dir().join(format!("vela_rec_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("bounds.json");
        std::fs::write(
            &f,
            r#"{"bounds":[
                {"n":7,"best_lower_bound":24,"accepted":true},
                {"n":24,"best_lower_bound":7179,"accepted":true},
                {"n":25,"best_lower_bound":9999,"accepted":false}
            ]}"#,
        )
        .unwrap();
        let best = read_records_best(&f).expect("some accepted records");
        assert_eq!(
            best["accepted_records"].as_i64(),
            Some(2),
            "unaccepted skipped"
        );
        assert_eq!(best["max_n"].as_i64(), Some(24), "deepest accepted n");
        assert_eq!(best["bound_at_max_n"].as_i64(), Some(7179));
        // absent / no-accepted -> None
        assert!(read_records_best(&dir.join("missing.json")).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn shipped_sidon_records_use_the_standard_catalog_path() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let current = repo.join("frontiers/sidon/records.json");
        let best = read_records_best(&current).expect("shipped Sidon records catalog");
        assert_eq!(best["accepted_records"].as_i64(), Some(18));
        assert_eq!(best["max_n"].as_i64(), Some(24));
        assert_eq!(best["bound_at_max_n"].as_i64(), Some(7179));
    }

    #[test]
    fn engine_kinds_cover_the_catalog_families() {
        // Every verifier_kind the HorizonMath catalog uses must be a real engine
        // kind (else `foundry targets` would mislabel it unattackable).
        for k in [
            "diff_triangle",
            "cap",
            "sidon",
            "gf2_sidon",
            "covering",
            "constant_weight",
            "costas",
            "union_free",
            "rook_directions",
            "bh",
            "golomb",
        ] {
            assert!(FOUNDRY_ENGINE_KINDS.contains(&k), "{k} must be attackable");
        }
    }

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
}
