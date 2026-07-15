//! `vela policy` — standing rules: the ceremony that pays compound interest.
//!
//! A key-holding human signs ONE scoped [`AcceptancePolicy`]; from then on the
//! deterministic evaluator routes every matching landing (`permit` lands the
//! canonical event, `defer` waits for `vela sign`, `deny` refuses). The porcelain
//! here is six verbs shaped as a ladder:
//!
//!   show    the active policy, its signature state, what it admitted lately
//!   draft   seal a policy from a template (no authority until signed)
//!   test    dry-run the policy over every pending proposal (never mutates)
//!   sign    THE ceremony: review, one confirm, one key read — the lane opens
//!   revoke  sign a causal head close; snapshots keep past events verifiable
//!   log     every policy-lane admission across all policies
//!
//! Custody doctrine is unchanged: agents draft, the evaluator routes, only a
//! human key opens a lane (`resolve_decision_actor` refuses `agent:`/`ci:` with
//! exit 4), and revocation is a real signed human review — never a model
//! output. One recoverable frontier transaction binds that review to the
//! signature-pointer change and retained snapshots. The signature verified
//! here is byte-for-byte the one [`load_active_policy`] checks before any
//! policy-lane accept.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::Utc;
use ed25519_dalek::Signer;
use serde_json::json;
use vela_protocol::acceptance_policy::{
    AcceptancePolicy, ActivePolicyMode, ActivePolicySnapshot, Constraints, EVALUATOR_VERSION,
    Outcome, PolicyRule, PolicySignatureRecord, Quorum, evaluate, load_active_policy,
    load_active_policy_snapshot,
};
use vela_protocol::cli_style as style;
use vela_protocol::project::Project;
use vela_protocol::proposals::StateProposal;
use vela_protocol::proposals::policy_accept::{
    CAUSALLY_UNBOUNDED_POLICY_EXPIRY, POLICY_HEAD_PROPOSAL_KIND, POLICY_HEAD_SCHEMA,
    POLICY_LANE_PAYLOAD_KEY, PolicyHead, PolicyHeadAction, PolicyHeadPayload, current_policy_head,
    verify_policy_lane_events,
};
use vela_protocol::repo;

use crate::cli::print_json;
use crate::ui::{self, ErrorKind, fail_with};

// ── Store layout ───────────────────────────────────────────────────────

fn policies_dir(frontier: &Path) -> PathBuf {
    frontier.join(".vela").join("policies")
}

fn active_path(frontier: &Path) -> PathBuf {
    policies_dir(frontier).join("active.json")
}

fn active_sig_path(frontier: &Path) -> PathBuf {
    policies_dir(frontier).join("active.sig.json")
}

fn revoked_marker_path(frontier: &Path, policy_id: &str) -> PathBuf {
    policies_dir(frontier).join(format!("revoked-{policy_id}.json"))
}

// ── Errors: testable cores return these; cmd_ wrappers fail_with them ──

#[derive(Debug)]
struct CmdError {
    kind: ErrorKind,
    message: String,
    hint: Option<String>,
}

impl CmdError {
    fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        CmdError {
            kind,
            message: message.into(),
            hint: None,
        }
    }

    fn hinted(kind: ErrorKind, message: impl Into<String>, hint: impl Into<String>) -> Self {
        CmdError {
            kind,
            message: message.into(),
            hint: Some(hint.into()),
        }
    }

    fn fail(self) -> ! {
        fail_with(self.kind, &self.message, self.hint.as_deref())
    }
}

// ── The template ladder ────────────────────────────────────────────────

const TEMPLATES: &str =
    "witness-rederivation, lean-rederivation, statement-drafts, search-witness, notes-threshold";

/// The hardcoded template ladder, ordered by how much a signature delegates.
/// Every template defaults to `Defer` (never a permit default) and uses signed
/// causal rotation/revocation as its validity boundary. An unsigned event
/// cannot prove it occurred inside a wall-clock expiry window, so generated
/// policies do not pretend a finite timestamp can authorize auto-Permit.
///
///   witness-rederivation  exact witnesses the frozen gate re-derived (A3,
///                         independent, method-sound, no claim-text change)
///   lean-rederivation     kernel-clean, axiom-audited Lean re-derivations
///                         (receipt_lean_kernel_clean, A2, method-sound).
///                         Signing it delegates the statement-fidelity call
///                         for the frontier; dirty/unlisted axioms defer.
///   statement-drafts      theoretical receipts from `vela land` (statement
///                         drafts land as receipt_theoretical) — drafts ARE
///                         text, so semantic text change is allowed, bounded
///                         to one finding at A2; independence is a
///                         verdict-time property, not a draft-time one
///   search-witness        computational receipts whose witness a FROZEN
///                         verifier already re-checked (`vela land`
///                         type=computational, assurance 2 = a passing
///                         verifier run). The autonomy lane for the harvest:
///                         a lower bound or a finite confirmation the machine
///                         can prove, landing without ceremony. No claim-text
///                         mutation and no dependents — a witness attests
///                         exactly its own assertion, nothing downstream.
///   notes-threshold       notes attach without ceremony (A0) but the impact
///                         constraints stay tight: one finding, no dependents,
///                         no claim-language mutation
fn template_policy(name: &str) -> Option<(Vec<PolicyRule>, &'static str)> {
    match name {
        "witness-rederivation" => Some((
            vec![PolicyRule {
                id: "witness-rederivation-v1".to_string(),
                effect: Outcome::Permit,
                claim_classes: vec![
                    "sidon_lower_bound".to_string(),
                    "witness_construction".to_string(),
                ],
                constraints: Constraints {
                    max_changed_findings: 1,
                    max_downstream_dependents: 5,
                    required_assurance_min: 3,
                    allow_semantic_text_change: false,
                    allow_contested: false,
                    allow_governance_mutation: false,
                    require_independence: true,
                    require_method_integrity: true,
                },
            }],
            "exact witnesses the frozen gate re-derived land themselves",
        )),
        "lean-rederivation" => Some((
            vec![PolicyRule {
                id: "lean-rederivation-v1".to_string(),
                effect: Outcome::Permit,
                // The class `vela land` stamps ONLY when a receipt's Lean runs
                // are kernel-clean under the frozen TCB policy (no sorryAx, no
                // compiler-trust axiom, nothing outside the allowlist). A Lean
                // receipt with a forbidden axiom lands as receipt_theoretical
                // and never matches this rule.
                claim_classes: vec!["receipt_lean_kernel_clean".to_string()],
                constraints: Constraints {
                    max_changed_findings: 1,
                    // A theorem attests exactly its own statement; nothing
                    // hangs downstream of a fresh re-derivation.
                    max_downstream_dependents: 0,
                    // A2 = at least one passing Lean run at land; the kernel
                    // re-derivation with an audited-clean axiom set is the
                    // un-forgeable floor this lane stands on (the Lean analogue
                    // of search-witness's frozen verifier).
                    required_assurance_min: 2,
                    // Every new receipt stamps assertion_text_mutated=true, so
                    // this must be open or the lane never fires. The real guard
                    // is require_method_integrity below — kernel-clean.
                    allow_semantic_text_change: true,
                    allow_contested: false,
                    allow_governance_mutation: false,
                    // Independence is a verdict-time property and single-relay
                    // re-derivations stamp it false; the kernel + audited axioms
                    // are the integrity this lane stands on.
                    require_independence: false,
                    require_method_integrity: true,
                },
            }],
            // Signing THIS policy is the fidelity delegation: the maintainer
            // decides, once and revocably, that for this frontier a kernel-clean
            // axiom-audited Lean re-derivation faithfully formalizes the claim,
            // so the per-item statement-fidelity call is delegated. Every
            // auto-admit still stamps a decision certificate; anything not
            // kernel-clean, or any dirty/unlisted axiom, defers to the human.
            "kernel-clean axiom-audited Lean re-derivations land at A2 (signing delegates fidelity)",
        )),
        "statement-drafts" => Some((
            vec![PolicyRule {
                id: "statement-drafts-v1".to_string(),
                effect: Outcome::Permit,
                // The class `vela land` actually stamps for statement
                // drafts (receipt_<type>); a class nothing produces would
                // make this a ceremony that delegates nothing.
                claim_classes: vec!["receipt_theoretical".to_string()],
                constraints: Constraints {
                    max_changed_findings: 1,
                    max_downstream_dependents: 0,
                    required_assurance_min: 2,
                    // Drafts ARE text: this lane exists to land statement
                    // language, so the semantic-text guard is deliberately open
                    // while everything else stays bounded to one finding.
                    allow_semantic_text_change: true,
                    allow_contested: false,
                    allow_governance_mutation: false,
                    // A draft is one agent's work by nature; independence
                    // is judged at verdict time, and landing stamps it
                    // false — requiring it here would permit nothing.
                    require_independence: false,
                    require_method_integrity: true,
                },
            }],
            "statement drafts (theoretical receipts) land at A2 (drafts ARE text)",
        )),
        "search-witness" => Some((
            vec![PolicyRule {
                id: "search-witness-v1".to_string(),
                effect: Outcome::Permit,
                // The class `vela land` stamps for a computational receipt
                // (type=computational → receipt_computational). The witness
                // was frozen-verifier-checked before the claim existed, so
                // the assurance-2 floor here IS that passing verifier run.
                claim_classes: vec!["receipt_computational".to_string()],
                constraints: Constraints {
                    max_changed_findings: 1,
                    // A witness attests exactly its own assertion (a bound, a
                    // finite confirmation); nothing hangs downstream of it.
                    max_downstream_dependents: 0,
                    // A2 = at least one passing frozen-verifier run at land.
                    required_assurance_min: 2,
                    // `vela land` stamps assertion_text_mutated=true for
                    // EVERY new receipt (a new claim is new text), so this
                    // must be open or the lane never fires — the same
                    // structural fact statement-drafts rides. The real guard
                    // for a witness is require_method_integrity below: the
                    // frozen verifier re-checked it. (Caught by the harness
                    // dry-run, which exercises the real land path.)
                    allow_semantic_text_change: true,
                    allow_contested: false,
                    allow_governance_mutation: false,
                    // Independence is a verdict-time property; the frozen
                    // verifier re-checking the witness IS the method integrity
                    // this lane stands on.
                    require_independence: false,
                    require_method_integrity: true,
                },
            }],
            "frozen-verified computational witnesses (bounds, finite confirmations) land at A2",
        )),
        "notes-threshold" => Some((
            vec![PolicyRule {
                id: "notes-threshold-v1".to_string(),
                effect: Outcome::Permit,
                claim_classes: vec!["finding_note".to_string()],
                constraints: Constraints {
                    max_changed_findings: 1,
                    max_downstream_dependents: 0,
                    required_assurance_min: 0,
                    allow_semantic_text_change: false,
                    allow_contested: false,
                    allow_governance_mutation: false,
                    require_independence: false,
                    require_method_integrity: false,
                },
            }],
            "notes attach without ceremony; anything touching claims defers",
        )),
        _ => None,
    }
}

/// Whether a named template's rules cover a claim class (suggest uses
/// this to prefer a reviewed, named shape over a bespoke rule).
pub(crate) fn template_covers_class(template: &str, class: &str) -> bool {
    template_policy(template)
        .map(|(rules, _)| {
            rules
                .iter()
                .any(|r| r.claim_classes.iter().any(|c| c == class))
        })
        .unwrap_or(false)
}

/// The first rule of a named template, for suggestion previews.
pub(crate) fn template_rule(template: &str) -> Option<PolicyRule> {
    template_policy(template).and_then(|(rules, _)| rules.into_iter().next())
}

/// Proposal ids the policy lane admitted (they never reached a human).
pub(crate) fn lane_admission_proposal_ids(project: &Project) -> std::collections::BTreeSet<String> {
    lane_admissions(project)
        .into_iter()
        .map(|a| a.proposal_id)
        .collect()
}

// ── Cores (testable; no process exits, no prompts) ─────────────────────

/// Read + integrity-check the sealed `active.json`. This is the shared
/// pre-step for `test` and `sign`: the file must exist, parse, and its
/// `vap_` id must re-derive from the body (a tampered draft never reaches
/// a signature or an evaluation).
fn read_sealed_active(frontier: &Path) -> Result<AcceptancePolicy, CmdError> {
    let path = active_path(frontier);
    if !path.exists() {
        return Err(CmdError::hinted(
            ErrorKind::NotFound,
            "no policy at .vela/policies/active.json — the lane is closed",
            format!("draft one: `vela policy draft <template>` (templates: {TEMPLATES})"),
        ));
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| CmdError::new(ErrorKind::Domain, format!("read {}: {e}", path.display())))?;
    let policy: AcceptancePolicy = serde_json::from_str(&raw)
        .map_err(|e| CmdError::new(ErrorKind::Domain, format!("active policy parse: {e}")))?;
    if !policy.id_is_valid() {
        return Err(CmdError::hinted(
            ErrorKind::Domain,
            format!(
                "active policy id does not re-derive (stored {}, sealed {})",
                policy.id,
                policy.content_address()
            ),
            "re-draft it: `vela policy draft <template> --replace`",
        ));
    }
    Ok(policy)
}

/// Seal a policy from a template into `.vela/policies/active.json`.
///
/// The id is the content address of the body (tamper-evident), the epoch is
/// the prior active policy's epoch + 1, validity is bounded by the signed
/// policy-head chain, and the default outcome is `Defer`. Sealing grants NO
/// authority — that is the signature's
/// job. Refuses to overwrite an existing SIGNED active policy unless
/// `replace` is set; replacing snapshots the outgoing pair content-addressed
/// (past admissions keep verifying) and closes the lane until the new draft
/// is signed. Returns the sealed policy and whether a signed policy was
/// rotated out.
fn draft_policy(
    frontier: &Path,
    template: &str,
    replace: bool,
) -> Result<(AcceptancePolicy, bool), CmdError> {
    let (rules, _) = template_policy(template).ok_or_else(|| {
        CmdError::hinted(
            ErrorKind::Usage,
            format!("unknown template `{template}`"),
            format!("templates: {TEMPLATES}"),
        )
    })?;
    seal_policy(frontier, rules, replace)
}

/// The sealing core, decoupled from the template ladder so suggested
/// rules and templates share one path. Same contract as before the
/// split: content-addressed id, epoch+1, Defer default, causal validity,
/// rotation carries prior rules and quorum forward.
fn seal_policy(
    frontier: &Path,
    rules: Vec<PolicyRule>,
    replace: bool,
) -> Result<(AcceptancePolicy, bool), CmdError> {
    let issued_by = crate::cli_identity::load_identity()
        .map(|identity| vec![identity.actor_id])
        .unwrap_or_default();
    seal_policy_with_issued_by(frontier, rules, replace, issued_by)
}

fn seal_policy_with_issued_by(
    frontier: &Path,
    rules: Vec<PolicyRule>,
    replace: bool,
    issued_by: Vec<String>,
) -> Result<(AcceptancePolicy, bool), CmdError> {
    let journal_dir = policy_transaction_journal_dir(frontier)?;
    let barrier = acquire_policy_draft_barrier(frontier, &journal_dir)?;
    let observed_snapshot = load_active_policy_snapshot(frontier)
        .map_err(|error| CmdError::new(ErrorKind::Domain, error))?;
    let request_fingerprint = policy_draft_request_fingerprint(&rules, replace, &issued_by)?;
    if observed_snapshot.mode == ActivePolicyMode::StagedUnsigned {
        let staged = read_sealed_active(frontier)?;
        let operation_id = policy_draft_operation_id(&staged.id);
        if let Some(plan) = barrier
            .completed_plan(&operation_id)
            .map_err(transaction_error)?
            && plan
                .result
                .get("schema")
                .and_then(serde_json::Value::as_str)
                == Some(POLICY_DRAFT_RESULT_SCHEMA)
            && plan
                .result
                .get("replacement_policy_id")
                .and_then(serde_json::Value::as_str)
                == Some(staged.id.as_str())
            && plan
                .result
                .get("request_fingerprint")
                .and_then(serde_json::Value::as_str)
                == Some(request_fingerprint.as_str())
            && let Some(replaced_signed) = plan
                .result
                .get("replaced_signed")
                .and_then(serde_json::Value::as_bool)
        {
            return Ok((staged, replaced_signed));
        }
    }
    let project =
        repo::load_from_path(frontier).map_err(|e| CmdError::new(ErrorKind::Domain, e))?;

    let prior = if observed_snapshot.policy_bytes.is_some() {
        Some(read_sealed_active(frontier)?)
    } else {
        None
    };
    let prior_epoch = prior.as_ref().map(|p| p.epoch).unwrap_or(0);

    let replaced_signed = observed_snapshot.mode == ActivePolicyMode::Active;
    if replaced_signed {
        let prior_id = prior
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| "unparseable".to_string());
        if !replace {
            return Err(CmdError::hinted(
                ErrorKind::Exists,
                format!(
                    "an active SIGNED policy exists ({prior_id}, epoch {prior_epoch}) — refusing \
                     to overwrite a live governance object"
                ),
                "pass --replace to rotate it (the lane closes until the new draft is signed)",
            ));
        }
    }

    // A rotation carries the standing grants forward: the new epoch is
    // the prior authority PLUS the template's rule, not a reset — a
    // signed lane must never close as a side effect of opening another.
    // A template rule with the same id supersedes (that IS the edit).
    let mut rules = rules;
    if let Some(old) = prior.as_ref().filter(|_| replaced_signed) {
        let new_ids: std::collections::HashSet<&str> =
            rules.iter().map(|r| r.id.as_str()).collect();
        let mut carried: Vec<PolicyRule> = old
            .rules
            .iter()
            .filter(|r| !new_ids.contains(r.id.as_str()))
            .cloned()
            .collect();
        carried.append(&mut rules);
        rules = carried;
    }
    let quorum = prior
        .as_ref()
        .filter(|_| replaced_signed)
        .map(|p| p.quorum.clone())
        .unwrap_or(Quorum {
            threshold: 1,
            // Proposal acceptance authority is registered under the
            // reviewer namespace. A steward-only policy must be an explicit
            // governance choice backed by a registered `steward:` actor; it
            // must never be accidentally satisfied by a reviewer signer.
            eligible_roles: vec!["reviewer".to_string()],
        });

    let mut policy = AcceptancePolicy {
        schema: "vela.acceptance_policy.v0.1".to_string(),
        id: String::new(),
        frontier_id: project.frontier_id.clone().unwrap_or_default(),
        epoch: prior_epoch
            .checked_add(1)
            .ok_or_else(|| transaction_error("policy epoch overflow"))?,
        issued_by,
        quorum,
        rules,
        default: Outcome::Defer,
        expires_at: CAUSALLY_UNBOUNDED_POLICY_EXPIRY.to_string(),
        revocation_ref: None,
    };
    policy.id = policy.content_address();
    if !policy.id_is_valid() {
        return Err(CmdError::new(
            ErrorKind::Domain,
            "internal: sealed id failed self-check",
        ));
    }

    install_policy_draft_transactional(
        frontier,
        barrier,
        &project,
        &observed_snapshot,
        &policy,
        &request_fingerprint,
        replaced_signed,
    )?;
    Ok((policy, replaced_signed))
}

/// Install an ordinary policy draft as an event-neutral frontier transaction.
/// Policy pointers are part of completed-history verification: changing them
/// with direct file I/O would make a prior signing or draft journal look
/// corrupt at the next recovery barrier. Recording the exact preimage ->
/// postimage edge keeps those completed journals auditable and supersedable.
fn install_policy_draft_transactional(
    frontier: &Path,
    barrier: crate::frontier_txn::FrontierRecoveryBarrier,
    project: &Project,
    observed: &ActivePolicySnapshot,
    replacement: &AcceptancePolicy,
    request_fingerprint: &crate::frontier_txn::ContentDigest,
    replaced_signed: bool,
) -> Result<(), CmdError> {
    use crate::frontier_txn::{
        ContentDigest, DeltaDraft, FrontierBinding, FrontierTxn, FrontierTxnPlan,
        FrontierTxnPlanSpec, InputBinding, OperationKind, PlannedWrite, RepoPath, WriteClass,
    };

    let mut replacement_bytes =
        serde_json::to_vec_pretty(replacement).map_err(transaction_error)?;
    replacement_bytes.push(b'\n');

    let mut writes = Vec::new();
    if replaced_signed {
        let prior = observed
            .verified
            .as_ref()
            .ok_or_else(|| transaction_error("active snapshot has no verified policy"))?;
        let policy_bytes = observed
            .policy_bytes
            .as_deref()
            .ok_or_else(|| transaction_error("active snapshot has no policy bytes"))?;
        let signature_bytes = observed
            .signature_bytes
            .as_deref()
            .ok_or_else(|| transaction_error("active snapshot has no signature bytes"))?;
        let policy_path = format!(".vela/policies/{}.json", prior.policy.id);
        let signature_path = format!(".vela/policies/{}.sig.json", prior.policy.id);
        require_exact_or_absent(frontier, &policy_path, policy_bytes)?;
        require_exact_or_absent(frontier, &signature_path, signature_bytes)?;
        writes.push(PlannedWrite::write(
            RepoPath::parse(policy_path).map_err(transaction_error)?,
            WriteClass::CanonicalEvidence,
            policy_bytes.to_vec(),
        ));
        writes.push(PlannedWrite::write(
            RepoPath::parse(signature_path).map_err(transaction_error)?,
            WriteClass::Authority,
            signature_bytes.to_vec(),
        ));
    }
    writes.push(PlannedWrite::write(
        RepoPath::parse(".vela/policies/active.json").map_err(transaction_error)?,
        WriteClass::CanonicalEvidence,
        replacement_bytes,
    ));
    if observed.signature_bytes.is_some() {
        writes.push(PlannedWrite::delete(
            RepoPath::parse(".vela/policies/active.sig.json").map_err(transaction_error)?,
            WriteClass::Authority,
        ));
    }

    let draft = DeltaDraft::prepare(frontier, writes).map_err(transaction_error)?;
    let layout = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.frontier-layout.internal.v1",
        "frontier_id": project.frontier_id(),
        "paths": draft
            .delta
            .writes()
            .iter()
            .map(|write| write.path.as_str())
            .collect::<Vec<_>>(),
    }))
    .map_err(transaction_error)?;
    let policy_preimage = observed.policy_bytes.as_deref().map(ContentDigest::hash);
    let signature_preimage = observed.signature_bytes.as_deref().map(ContentDigest::hash);
    let intent = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.policy-draft-intent.internal.v1",
        "frontier_id": project.frontier_id(),
        "replacement_policy_id": replacement.id,
        "request_fingerprint": request_fingerprint,
        "policy_preimage": policy_preimage,
        "signature_preimage": signature_preimage,
    }))
    .map_err(transaction_error)?;
    let request_root = ContentDigest::hash(intent);
    let operation_id = policy_draft_operation_id(&replacement.id);
    let event_log_root = ContentDigest::parse(format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    ))
    .map_err(transaction_error)?;
    let mut resulting_event_ids = project
        .events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    resulting_event_ids.sort();
    let read_set = vec![
        InputBinding::project_snapshot(project).map_err(transaction_error)?,
        InputBinding::file_snapshot(
            RepoPath::parse(".vela/policies/active.json").map_err(transaction_error)?,
            observed.policy_bytes.as_deref(),
        )
        .map_err(transaction_error)?,
        InputBinding::file_snapshot(
            RepoPath::parse(".vela/policies/active.sig.json").map_err(transaction_error)?,
            observed.signature_bytes.as_deref(),
        )
        .map_err(transaction_error)?,
    ];
    barrier
        .verify_read_set(&read_set)
        .map_err(transaction_error)?;
    let plan = FrontierTxnPlan::new(
        FrontierTxnPlanSpec {
            kind: OperationKind::Maintenance,
            operation_id,
            request_root,
            frontier: FrontierBinding::new(frontier, project.frontier_id(), &layout)
                .map_err(transaction_error)?,
            fixed_time: Utc::now().to_rfc3339(),
            expected_event_log_root: event_log_root.clone(),
            resulting_event_log_root: event_log_root,
            resulting_event_ids,
            read_set,
            result: json!({
                "schema": POLICY_DRAFT_RESULT_SCHEMA,
                "state": "staged_unsigned",
                "replacement_policy_id": replacement.id,
                "request_fingerprint": request_fingerprint,
                "replaced_signed": replaced_signed,
            }),
        },
        draft.delta.clone(),
    )
    .map_err(transaction_error)?;
    let mut transaction =
        FrontierTxn::prepare_with_barrier(barrier, plan, draft).map_err(transaction_error)?;
    transaction.mark_committed().map_err(transaction_error)?;
    install_policy_draft(&mut transaction).map_err(transaction_error)?;
    transaction.complete().map_err(transaction_error)?;

    let installed = load_active_policy_snapshot(frontier).map_err(transaction_error)?;
    if installed.mode != ActivePolicyMode::StagedUnsigned || active_sig_path(frontier).exists() {
        return Err(transaction_error(
            "completed policy draft did not leave one unsigned active draft",
        ));
    }
    if read_sealed_active(frontier)? != *replacement {
        return Err(transaction_error(
            "completed policy draft installed a different replacement policy",
        ));
    }
    Ok(())
}

const POLICY_DRAFT_RESULT_SCHEMA: &str = "vela.policy-draft-result.internal.v1";

fn policy_draft_operation_id(replacement_policy_id: &str) -> crate::frontier_txn::OperationId {
    crate::frontier_txn::OperationId::derive("policy-draft", replacement_policy_id.as_bytes())
}

fn policy_draft_request_fingerprint(
    rules: &[PolicyRule],
    replace: bool,
    issued_by: &[String],
) -> Result<crate::frontier_txn::ContentDigest, CmdError> {
    let intent = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.policy-draft-request.internal.v1",
        "rules": rules,
        "replace": replace,
        "issued_by": issued_by,
    }))
    .map_err(transaction_error)?;
    Ok(crate::frontier_txn::ContentDigest::hash(intent))
}

#[cfg(test)]
std::thread_local! {
    static POLICY_DRAFT_INSTALL_FAILPOINT:
        std::cell::Cell<Option<crate::frontier_txn::FrontierTxnStep>> = const {
            std::cell::Cell::new(None)
        };
}

#[cfg(test)]
fn set_policy_draft_install_failpoint(step: Option<crate::frontier_txn::FrontierTxnStep>) {
    POLICY_DRAFT_INSTALL_FAILPOINT.with(|failpoint| failpoint.set(step));
}

fn install_policy_draft(
    transaction: &mut crate::frontier_txn::FrontierTxn,
) -> Result<(), crate::frontier_txn::FrontierTxnError> {
    #[cfg(test)]
    if let Some(step) = POLICY_DRAFT_INSTALL_FAILPOINT.with(std::cell::Cell::take) {
        return transaction.install_at_failpoint(step);
    }
    transaction.install()
}

fn require_exact_or_absent(
    frontier: &Path,
    relative: &str,
    expected: &[u8],
) -> Result<(), CmdError> {
    let path = frontier.join(relative);
    match std::fs::read(&path) {
        Ok(existing) if existing == expected => Ok(()),
        Ok(_) => Err(transaction_error(format!(
            "policy snapshot {} already exists with different bytes",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(transaction_error(format!(
            "read policy snapshot {}: {error}",
            path.display()
        ))),
    }
}

/// The signing core: Ed25519 over a domain-separated canonical envelope that
/// binds the sealed policy and `signed_at`. Writes `active.sig.json` as a
/// [`PolicySignatureRecord`] plus the content-addressed snapshot pair
/// `<vap_id>.json` / `<vap_id>.sig.json`, then round-trips the loader to
/// prove the lane actually opened. Refuses a revoked policy (revocation is
/// not undone by re-signing) and is idempotent on an already-open lane.
/// Test-only frozen file-format fixture: production must use the transactional
/// ceremony below so no callable file-direct authority bypass exists.
#[cfg(test)]
fn sign_active_policy(
    frontier: &Path,
    key: &ed25519_dalek::SigningKey,
    signed_at: &str,
) -> Result<(AcceptancePolicy, PolicySignatureRecord), CmdError> {
    let policy = read_sealed_active(frontier)?;
    if revoked_marker_path(frontier, &policy.id).exists() {
        return Err(CmdError::hinted(
            ErrorKind::Custody,
            format!(
                "{} was revoked — a revoked policy is never re-signed",
                policy.id
            ),
            "draft a new epoch: `vela policy draft <template>`",
        ));
    }
    if let Ok(Some(_)) = load_active_policy(frontier) {
        return Err(CmdError::hinted(
            ErrorKind::Exists,
            format!("{} is already signed — the lane is open", policy.id),
            "rotate deliberately: `vela policy revoke --reason <why>` then draft + sign",
        ));
    }

    let body = vela_protocol::acceptance_policy::policy_signature_preimage(&policy, signed_at)
        .map_err(|e| CmdError::new(ErrorKind::Domain, format!("canonical: {e}")))?;
    let sig = key.sign(&body);
    let record = PolicySignatureRecord {
        policy_id: policy.id.clone(),
        signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
        signature: hex::encode(sig.to_bytes()),
        signed_at: signed_at.to_string(),
    };
    let sig_path = active_sig_path(frontier);
    let record_body = serde_json::to_string_pretty(&record)
        .map_err(|e| CmdError::new(ErrorKind::Domain, format!("serialize signature: {e}")))?;
    std::fs::write(&sig_path, format!("{record_body}\n")).map_err(|e| {
        CmdError::new(
            ErrorKind::Domain,
            format!("write {}: {e}", sig_path.display()),
        )
    })?;

    // Content-addressed snapshots: the pair that keeps every event this
    // policy admits verifiable after `active.json` rotates.
    let dir = policies_dir(frontier);
    let snap = dir.join(format!("{}.json", policy.id));
    let snap_sig = dir.join(format!("{}.sig.json", policy.id));
    if !snap.exists() {
        std::fs::copy(active_path(frontier), &snap).map_err(|e| {
            CmdError::new(
                ErrorKind::Domain,
                format!("snapshot {}: {e}", snap.display()),
            )
        })?;
    }
    std::fs::copy(&sig_path, &snap_sig).map_err(|e| {
        CmdError::new(
            ErrorKind::Domain,
            format!("snapshot {}: {e}", snap_sig.display()),
        )
    })?;

    // The proof of the ceremony: the exact loader the accept lane uses must
    // see an open lane, or the signature we just wrote is worthless.
    match load_active_policy(frontier) {
        Ok(Some(_)) => Ok((policy, record)),
        Ok(None) => Err(CmdError::new(
            ErrorKind::Domain,
            "signature written but the loader does not see an open lane",
        )),
        Err(e) => Err(CmdError::new(
            ErrorKind::Domain,
            format!("post-sign verification failed: {e}"),
        )),
    }
}

/// The revocation core: the `active.sig.json` pointer loses authority (the
/// lane closes) while the content-addressed snapshots STAY so every event the
/// policy admitted keeps verifying on replay. Writes a
/// `revoked-<vap_id>.json` marker that `sign` honors — re-signing never
/// resurrects a revoked policy. Returns the revoked `vap_` id.
/// Test-only frozen file-format fixture; production revocation is the keyed,
/// recoverable policy-head transaction below.
#[cfg(test)]
fn revoke_active_policy(
    frontier: &Path,
    actor: &str,
    reason: &str,
    revoked_at: &str,
) -> Result<String, CmdError> {
    let policy = read_sealed_active(frontier)?;
    let sig_path = active_sig_path(frontier);
    if !sig_path.exists() {
        return Err(CmdError::new(
            ErrorKind::Exists,
            format!(
                "the lane is already closed — {} carries no signature",
                policy.id
            ),
        ));
    }

    // Replay insurance first: never delete the only copy of the signature
    // that past policy-lane events verify against.
    let dir = policies_dir(frontier);
    let snap = dir.join(format!("{}.json", policy.id));
    let snap_sig = dir.join(format!("{}.sig.json", policy.id));
    if !snap.exists() {
        std::fs::copy(active_path(frontier), &snap).map_err(|e| {
            CmdError::new(
                ErrorKind::Domain,
                format!("snapshot {}: {e}", snap.display()),
            )
        })?;
    }
    if !snap_sig.exists() {
        std::fs::copy(&sig_path, &snap_sig).map_err(|e| {
            CmdError::new(
                ErrorKind::Domain,
                format!("snapshot {}: {e}", snap_sig.display()),
            )
        })?;
    }
    std::fs::remove_file(&sig_path).map_err(|e| {
        CmdError::new(
            ErrorKind::Domain,
            format!("remove {}: {e}", sig_path.display()),
        )
    })?;

    let marker = revoked_marker_path(frontier, &policy.id);
    let record = json!({
        "schema": "vela.policy_revocation.v0.1",
        "policy_id": policy.id,
        "revoked_at": revoked_at,
        "revoked_by": actor,
        "reason": reason,
    });
    let body = serde_json::to_string_pretty(&record)
        .map_err(|e| CmdError::new(ErrorKind::Domain, format!("serialize revocation: {e}")))?;
    std::fs::write(&marker, format!("{body}\n")).map_err(|e| {
        CmdError::new(
            ErrorKind::Domain,
            format!("write {}: {e}", marker.display()),
        )
    })?;
    Ok(policy.id)
}

/// Worktree-private journal used by the same recoverable frontier transaction
/// barrier as receipt landing and proposal creation. Policy ceremony journals
/// contain public postimages only; private key bytes never enter them.
fn policy_transaction_journal_dir(frontier: &Path) -> Result<PathBuf, CmdError> {
    let root = frontier
        .canonicalize()
        .map_err(|error| CmdError::new(ErrorKind::Domain, format!("resolve frontier: {error}")))?;
    let vela = root.join(".vela");
    let metadata = std::fs::symlink_metadata(&vela).map_err(|error| {
        CmdError::new(
            ErrorKind::Domain,
            format!(
                "inspect private frontier directory {}: {error}",
                vela.display()
            ),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CmdError::new(
            ErrorKind::Domain,
            format!(
                "private frontier directory must be a real directory: {}",
                vela.display()
            ),
        ));
    }
    let journal = vela.join("operation-journals");
    match std::fs::symlink_metadata(&journal) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(CmdError::new(
                ErrorKind::Domain,
                format!(
                    "frontier transaction journal must be a real directory: {}",
                    journal.display()
                ),
            ))
        }
        Ok(_) => Ok(journal),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(journal),
        Err(error) => Err(CmdError::new(
            ErrorKind::Domain,
            format!(
                "inspect frontier transaction journal {}: {error}",
                journal.display()
            ),
        )),
    }
}

fn transaction_error(error: impl std::fmt::Display) -> CmdError {
    CmdError::new(
        ErrorKind::Domain,
        format!("policy-head transaction failed: {error}"),
    )
}

/// Reach one stable frontier barrier before inspecting or writing policy
/// pointers. A committed operation is deterministic and therefore safe to
/// finish; a marker-free prepared plan is aborted and replanned from current
/// bytes. No policy draft may route around either state with direct file I/O.
fn acquire_policy_draft_barrier(
    frontier: &Path,
    journal_dir: &Path,
) -> Result<crate::frontier_txn::FrontierRecoveryBarrier, CmdError> {
    use crate::frontier_txn::{FrontierTxn, FrontierTxnError, OperationId, RecoveryOutcome};

    for _ in 0..3 {
        match FrontierTxn::acquire_recovery_barrier(frontier, journal_dir) {
            Ok(barrier) => return Ok(barrier),
            Err(FrontierTxnError::RecoveryRequired { operation_id, .. }) => {
                let operation_id = OperationId::parse(operation_id).map_err(transaction_error)?;
                match FrontierTxn::recover(frontier, journal_dir, &operation_id)
                    .map_err(transaction_error)?
                {
                    RecoveryOutcome::Prepared => {
                        let mut transaction =
                            FrontierTxn::open(frontier, journal_dir, &operation_id)
                                .map_err(transaction_error)?;
                        transaction.abort_prepared().map_err(transaction_error)?;
                    }
                    RecoveryOutcome::Aborted
                    | RecoveryOutcome::Completed
                    | RecoveryOutcome::AlreadyCompleted => {}
                }
            }
            Err(error) => return Err(transaction_error(error)),
        }
    }
    Err(transaction_error(
        "frontier recovery did not reach a stable policy-draft barrier",
    ))
}

fn policy_ceremony_identity(
    frontier_id: &str,
    verb: &str,
    expected_policy_id: &str,
    actor: &str,
    reason: &str,
) -> Result<
    (
        crate::frontier_txn::ContentDigest,
        crate::frontier_txn::OperationId,
    ),
    CmdError,
> {
    use crate::frontier_txn::{ContentDigest, OperationId};
    let intent = json!({
        "schema": "vela.policy-head-ceremony-intent.internal.v1",
        "frontier_id": frontier_id,
        "verb": verb,
        "expected_policy_id": expected_policy_id,
        "actor": actor,
        "reason": reason,
    });
    let bytes = vela_protocol::canonical::to_canonical_bytes(&intent).map_err(transaction_error)?;
    let request_root = ContentDigest::hash(bytes);
    let operation_id = OperationId::derive("policy-head", request_root.as_str().as_bytes());
    Ok((request_root, operation_id))
}

fn resume_policy_ceremony(
    frontier: &Path,
    journal_dir: &Path,
    operation_id: &crate::frontier_txn::OperationId,
    request_root: &crate::frontier_txn::ContentDigest,
) -> Result<Option<serde_json::Value>, CmdError> {
    use crate::frontier_txn::{FrontierTxn, RecoveryState};
    let Some(mut transaction) = FrontierTxn::open_if_present(frontier, journal_dir, operation_id)
        .map_err(transaction_error)?
    else {
        return Ok(None);
    };
    if transaction.plan().request_root != *request_root {
        return Err(transaction_error(format!(
            "operation {} is bound to a different policy ceremony intent",
            operation_id.as_str()
        )));
    }
    if matches!(transaction.recovery_state(), RecoveryState::Aborted) {
        return Ok(None);
    }
    let result = transaction.plan().result.clone();
    if !matches!(transaction.recovery_state(), RecoveryState::Completed) {
        transaction.mark_committed().map_err(transaction_error)?;
        transaction.install().map_err(transaction_error)?;
        transaction.complete().map_err(transaction_error)?;
    }
    Ok(Some(result))
}

fn policy_head_from_transaction_result(
    frontier: &Path,
    result: &serde_json::Value,
) -> Result<PolicyHead, CmdError> {
    let event_id = result
        .get("head_event_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| transaction_error("policy ceremony result has no head_event_id"))?;
    vela_protocol::proposals::policy_accept::derive_policy_head_chain(
        &repo::load_from_path(frontier).map_err(transaction_error)?,
    )
    .map_err(transaction_error)?
    .into_iter()
    .find(|head| head.event_id == event_id)
    .ok_or_else(|| transaction_error("recovered policy-head event is outside the signed chain"))
}

#[cfg(test)]
std::thread_local! {
    /// 1 = after Prepared journal, before marker; 2 = after durable marker.
    static POLICY_CEREMONY_FAILPOINT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn set_policy_ceremony_failpoint(value: u8) {
    POLICY_CEREMONY_FAILPOINT.with(|failpoint| failpoint.set(value));
}

#[cfg(test)]
fn hit_policy_ceremony_failpoint(value: u8) -> Result<(), CmdError> {
    if POLICY_CEREMONY_FAILPOINT.with(|failpoint| failpoint.get()) == value {
        return Err(transaction_error(format!(
            "injected policy ceremony failure {value}"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn commit_policy_head_transaction(
    frontier: &Path,
    barrier: crate::frontier_txn::FrontierRecoveryBarrier,
    original: &Project,
    actor: &str,
    key: &ed25519_dalek::SigningKey,
    reason: &str,
    fixed_at: &str,
    action: PolicyHeadAction,
    policy_id: Option<String>,
    request_root: crate::frontier_txn::ContentDigest,
    operation_id: crate::frontier_txn::OperationId,
    extra_writes: Vec<crate::frontier_txn::PlannedWrite>,
    read_set: Vec<crate::frontier_txn::InputBinding>,
) -> Result<PolicyHead, CmdError> {
    use crate::frontier_txn::{
        ContentDigest, DeltaDraft, FrontierBinding, FrontierTxn, FrontierTxnPlan,
        FrontierTxnPlanSpec, OperationKind, PlannedWrite,
    };

    let current = current_policy_head(original).map_err(transaction_error)?;
    let (epoch, prior_head_event_id) = match (action, current.as_ref()) {
        (PolicyHeadAction::Activate, None) => (1, None),
        (PolicyHeadAction::Rotate | PolicyHeadAction::Revoke, Some(head)) => (
            head.epoch
                .checked_add(1)
                .ok_or_else(|| transaction_error("policy-head epoch overflow"))?,
            Some(head.event_id.clone()),
        ),
        _ => {
            return Err(transaction_error(
                "policy-head action does not extend the current signed head",
            ));
        }
    };
    if action == PolicyHeadAction::Rotate
        && current.as_ref().and_then(|head| head.policy_id.as_ref()) == policy_id.as_ref()
    {
        return Err(transaction_error(
            "policy-head rotation must name a different policy",
        ));
    }
    if action == PolicyHeadAction::Revoke {
        let Some(head) = current.as_ref() else {
            return Err(transaction_error(
                "cannot revoke without an active policy-head",
            ));
        };
        if head.action == PolicyHeadAction::Revoke || head.policy_id.is_none() {
            return Err(transaction_error(
                "the signed policy-head is already revoked",
            ));
        }
    }

    let parent_event_ids = original
        .events
        .iter()
        .map(|event| event.id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let expected_parent_event_log_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&original.events)
    );
    let proposal = vela_protocol::proposals::new_proposal_at(
        POLICY_HEAD_PROPOSAL_KIND,
        vela_protocol::events::StateTarget {
            r#type: "governance".to_string(),
            id: original.frontier_id().to_string(),
        },
        actor,
        "human",
        reason,
        serde_json::to_value(PolicyHeadPayload {
            schema: POLICY_HEAD_SCHEMA.to_string(),
            action,
            policy_id: policy_id.clone(),
            prior_head_event_id,
            expected_parent_event_log_root,
            parent_event_ids,
            epoch,
        })
        .map_err(transaction_error)?,
        Vec::new(),
        Vec::new(),
        fixed_at,
    );
    let proposal_id = proposal.id.clone();
    let mut candidate: Project =
        serde_json::from_value(serde_json::to_value(original).map_err(transaction_error)?)
            .map_err(transaction_error)?;
    vela_protocol::proposals::insert_pending_in_frontier(&mut candidate, proposal)
        .map_err(transaction_error)?;
    let head_event_id = vela_protocol::proposals::accept_policy_head_proposal_in_frontier_at(
        &mut candidate,
        &proposal_id,
        actor,
        reason,
        key,
        fixed_at,
    )
    .map_err(transaction_error)?;
    vela_protocol::project::recompute_stats(&mut candidate);
    let head = current_policy_head(&candidate)
        .map_err(transaction_error)?
        .ok_or_else(|| transaction_error("signed review did not derive a policy-head"))?;
    if head.event_id != head_event_id || head.action != action || head.policy_id != policy_id {
        return Err(transaction_error(
            "signed policy-head postimage does not match the ceremony intent",
        ));
    }

    let mut writes = PlannedWrite::from_managed_files(
        repo::render_vela_repo_files(frontier, &candidate).map_err(transaction_error)?,
    )
    .map_err(transaction_error)?;
    writes.extend(extra_writes);
    let draft = DeltaDraft::prepare(frontier, writes).map_err(transaction_error)?;
    let layout = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.frontier-layout.internal.v1",
        "frontier_id": original.frontier_id(),
        "paths": draft
            .delta
            .writes()
            .iter()
            .map(|write| write.path.as_str())
            .collect::<Vec<_>>(),
    }))
    .map_err(transaction_error)?;
    let expected_event_log_root = ContentDigest::parse(format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&original.events)
    ))
    .map_err(transaction_error)?;
    let resulting_event_log_root = ContentDigest::parse(format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&candidate.events)
    ))
    .map_err(transaction_error)?;
    let mut resulting_event_ids = candidate
        .events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    resulting_event_ids.sort();
    let result = json!({
        "policy_id": head.policy_id,
        "head_event_id": head.event_id,
        "head_epoch": head.epoch,
        "action": head.action,
    });
    let plan = FrontierTxnPlan::new(
        FrontierTxnPlanSpec {
            kind: OperationKind::Decision,
            operation_id,
            request_root,
            frontier: FrontierBinding::new(frontier, original.frontier_id(), &layout)
                .map_err(transaction_error)?,
            fixed_time: fixed_at.to_string(),
            expected_event_log_root,
            resulting_event_log_root,
            resulting_event_ids,
            read_set,
            result,
        },
        draft.delta.clone(),
    )
    .map_err(transaction_error)?;
    let mut transaction =
        FrontierTxn::prepare_with_barrier(barrier, plan, draft).map_err(transaction_error)?;
    #[cfg(test)]
    hit_policy_ceremony_failpoint(1)?;
    transaction.mark_committed().map_err(transaction_error)?;
    #[cfg(test)]
    hit_policy_ceremony_failpoint(2)?;
    transaction.install().map_err(transaction_error)?;
    transaction.complete().map_err(transaction_error)?;
    Ok(head)
}

/// One key read, one fixed instant, one recoverable transaction: the human
/// signs the policy envelope and the causal `review.accepted` head event with
/// the same in-memory key. `active.json` remains only the selected draft.
fn sign_active_policy_transactional<C, K>(
    frontier: &Path,
    actor: &str,
    expected_policy_id: &str,
    clock: C,
    load_key: K,
) -> Result<(AcceptancePolicy, PolicySignatureRecord, PolicyHead), CmdError>
where
    C: FnOnce() -> String,
    K: FnOnce() -> ed25519_dalek::SigningKey,
{
    use crate::frontier_txn::{FrontierTxn, InputBinding, PlannedWrite, RepoPath, WriteClass};

    const REASON: &str = "activate signed acceptance policy";
    let journal_dir = policy_transaction_journal_dir(frontier)?;
    let observed = repo::load_from_path(frontier).map_err(transaction_error)?;
    let (request_root, operation_id) = policy_ceremony_identity(
        &observed.frontier_id(),
        "sign",
        expected_policy_id,
        actor,
        REASON,
    )?;
    if let Some(result) =
        resume_policy_ceremony(frontier, &journal_dir, &operation_id, &request_root)?
    {
        let policy = read_sealed_active(frontier)?;
        if policy.id != expected_policy_id {
            return Err(transaction_error(
                "recovered signing ceremony no longer matches the selected policy",
            ));
        }
        if revoked_marker_path(frontier, &policy.id).exists() {
            return Err(CmdError::hinted(
                ErrorKind::Custody,
                format!(
                    "{} was revoked — a completed old signing ceremony cannot resurrect it",
                    policy.id
                ),
                "draft and sign a new policy epoch",
            ));
        }
        let record: PolicySignatureRecord = serde_json::from_slice(
            &std::fs::read(active_sig_path(frontier)).map_err(transaction_error)?,
        )
        .map_err(transaction_error)?;
        let head = policy_head_from_transaction_result(frontier, &result)?;
        return Ok((policy, record, head));
    }
    let barrier =
        FrontierTxn::acquire_recovery_barrier(frontier, &journal_dir).map_err(transaction_error)?;
    let original = repo::load_from_path(frontier).map_err(transaction_error)?;
    let policy = read_sealed_active(frontier)?;
    if policy.id != expected_policy_id {
        return Err(transaction_error(format!(
            "displayed policy {expected_policy_id} changed to {} before the signing barrier",
            policy.id
        )));
    }
    if revoked_marker_path(frontier, &policy.id).exists() {
        return Err(CmdError::hinted(
            ErrorKind::Custody,
            format!(
                "{} was revoked — a revoked policy is never re-signed",
                policy.id
            ),
            "draft a new epoch: `vela policy draft <template>`",
        ));
    }
    let current = current_policy_head(&original).map_err(transaction_error)?;
    if let Some(verified) = load_active_policy(frontier).map_err(transaction_error)? {
        if verified.policy.id == policy.id
            && let Some(head) = current.as_ref()
            && head.action != PolicyHeadAction::Revoke
            && head.policy_id.as_ref() == Some(&policy.id)
        {
            let record: PolicySignatureRecord = serde_json::from_slice(
                &std::fs::read(active_sig_path(frontier)).map_err(transaction_error)?,
            )
            .map_err(transaction_error)?;
            return Ok((policy, record, head.clone()));
        }
        return Err(transaction_error(
            "active policy signature and signed policy-head do not select the same policy",
        ));
    }
    if current.as_ref().is_some_and(|head| {
        head.action != PolicyHeadAction::Revoke && head.policy_id.as_ref() == Some(&policy.id)
    }) {
        return Err(transaction_error(
            "selected policy already has an active signed head but its active signature is missing",
        ));
    }
    let action = if current.is_none() {
        PolicyHeadAction::Activate
    } else {
        PolicyHeadAction::Rotate
    };
    let signed_at = clock();
    chrono::DateTime::parse_from_rfc3339(&signed_at)
        .map_err(|error| transaction_error(format!("policy signing clock is invalid: {error}")))?;
    // The recovery barrier is held and every mutable authority input above is
    // reloaded before this single key-loader call. The same in-memory key then
    // signs both the policy envelope and the causal review event below.
    let key = load_key();
    let body = vela_protocol::acceptance_policy::policy_signature_preimage(&policy, &signed_at)
        .map_err(transaction_error)?;
    let signature = key.sign(&body);
    let record = PolicySignatureRecord {
        policy_id: policy.id.clone(),
        signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
        signature: hex::encode(signature.to_bytes()),
        signed_at: signed_at.clone(),
    };
    let active_bytes = std::fs::read(active_path(frontier)).map_err(transaction_error)?;
    let mut signature_bytes = serde_json::to_vec_pretty(&record).map_err(transaction_error)?;
    signature_bytes.push(b'\n');
    let policy_path = format!(".vela/policies/{}.json", policy.id);
    let signature_path = format!(".vela/policies/{}.sig.json", policy.id);
    let writes = vec![
        PlannedWrite::write(
            RepoPath::parse(".vela/policies/active.sig.json").map_err(transaction_error)?,
            WriteClass::Authority,
            signature_bytes.clone(),
        ),
        PlannedWrite::write(
            RepoPath::parse(policy_path).map_err(transaction_error)?,
            WriteClass::CanonicalEvidence,
            active_bytes,
        ),
        PlannedWrite::write(
            RepoPath::parse(signature_path).map_err(transaction_error)?,
            WriteClass::Authority,
            signature_bytes,
        ),
    ];
    let read_set = vec![
        InputBinding::existing_file(
            frontier,
            RepoPath::parse(".vela/policies/active.json").map_err(transaction_error)?,
        )
        .map_err(transaction_error)?,
        InputBinding::existing_file(
            frontier,
            RepoPath::parse(".vela/actors.json").map_err(transaction_error)?,
        )
        .map_err(transaction_error)?,
    ];
    let head = commit_policy_head_transaction(
        frontier,
        barrier,
        &original,
        actor,
        &key,
        REASON,
        &signed_at,
        action,
        Some(policy.id.clone()),
        request_root,
        operation_id,
        writes,
        read_set,
    )?;
    let verified = load_active_policy(frontier)
        .map_err(transaction_error)?
        .ok_or_else(|| transaction_error("completed ceremony did not open the policy loader"))?;
    if verified.policy.id != policy.id {
        return Err(transaction_error(
            "completed ceremony opened a different active policy",
        ));
    }
    Ok((policy, record, head))
}

fn revoke_active_policy_transactional<C, K>(
    frontier: &Path,
    actor: &str,
    expected_policy_id: &str,
    clock: C,
    load_key: K,
    reason: &str,
) -> Result<(String, PolicyHead), CmdError>
where
    C: FnOnce() -> String,
    K: FnOnce() -> ed25519_dalek::SigningKey,
{
    use crate::frontier_txn::{FrontierTxn, InputBinding, PlannedWrite, RepoPath, WriteClass};

    let journal_dir = policy_transaction_journal_dir(frontier)?;
    let observed = repo::load_from_path(frontier).map_err(transaction_error)?;
    let (request_root, operation_id) = policy_ceremony_identity(
        &observed.frontier_id(),
        "revoke",
        expected_policy_id,
        actor,
        reason,
    )?;
    if let Some(result) =
        resume_policy_ceremony(frontier, &journal_dir, &operation_id, &request_root)?
    {
        let policy = read_sealed_active(frontier)?;
        if policy.id != expected_policy_id {
            return Err(transaction_error(
                "recovered revocation no longer matches the selected policy",
            ));
        }
        let head = policy_head_from_transaction_result(frontier, &result)?;
        let marker: serde_json::Value = serde_json::from_slice(
            &std::fs::read(revoked_marker_path(frontier, &policy.id)).map_err(transaction_error)?,
        )
        .map_err(transaction_error)?;
        if marker.get("revoked_by").and_then(serde_json::Value::as_str) != Some(actor)
            || marker.get("reason").and_then(serde_json::Value::as_str) != Some(reason)
        {
            return Err(transaction_error(
                "recovered revocation marker does not match this actor/reason intent",
            ));
        }
        return Ok((policy.id, head));
    }
    let barrier =
        FrontierTxn::acquire_recovery_barrier(frontier, &journal_dir).map_err(transaction_error)?;
    let original = repo::load_from_path(frontier).map_err(transaction_error)?;
    let policy = read_sealed_active(frontier)?;
    if policy.id != expected_policy_id {
        return Err(transaction_error(format!(
            "displayed policy {expected_policy_id} changed to {} before the revocation barrier",
            policy.id
        )));
    }
    let current = current_policy_head(&original).map_err(transaction_error)?;
    if let Some(head) = current.as_ref()
        && head.action == PolicyHeadAction::Revoke
    {
        let marker_path = revoked_marker_path(frontier, &policy.id);
        let marker: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&marker_path).map_err(|error| {
                transaction_error(format!(
                    "signed revoke head has no marker {}: {error}",
                    marker_path.display()
                ))
            })?)
            .map_err(transaction_error)?;
        if marker.get("policy_id").and_then(serde_json::Value::as_str) == Some(policy.id.as_str())
            && marker.get("revoked_by").and_then(serde_json::Value::as_str) == Some(actor)
            && marker.get("reason").and_then(serde_json::Value::as_str) == Some(reason)
        {
            return Ok((policy.id, head.clone()));
        }
        return Err(transaction_error(
            "existing signed revocation does not match this actor/reason intent",
        ));
    }
    let verified = load_active_policy(frontier)
        .map_err(transaction_error)?
        .ok_or_else(|| {
            CmdError::new(
                ErrorKind::Exists,
                format!(
                    "the lane is already closed — {} carries no signature",
                    policy.id
                ),
            )
        })?;
    if verified.policy.id != policy.id {
        return Err(transaction_error(
            "selected policy and verified active policy do not match",
        ));
    }
    if !current.as_ref().is_some_and(|head| {
        head.action != PolicyHeadAction::Revoke && head.policy_id.as_ref() == Some(&policy.id)
    }) {
        return Err(transaction_error(
            "active policy is not authorized by the current signed policy-head",
        ));
    }
    let revoked_at = clock();
    chrono::DateTime::parse_from_rfc3339(&revoked_at).map_err(|error| {
        transaction_error(format!("policy revocation clock is invalid: {error}"))
    })?;
    // Revocation is now a real keyed decision. Resolve the key exactly once,
    // only after the barrier and authority revalidation above.
    let key = load_key();
    let active_bytes = std::fs::read(active_path(frontier)).map_err(transaction_error)?;
    let signature_bytes = std::fs::read(active_sig_path(frontier)).map_err(transaction_error)?;
    let marker = json!({
        "schema": "vela.policy_revocation.v0.1",
        "policy_id": policy.id,
        "revoked_at": &revoked_at,
        "revoked_by": actor,
        "reason": reason,
    });
    let mut marker_bytes = serde_json::to_vec_pretty(&marker).map_err(transaction_error)?;
    marker_bytes.push(b'\n');
    let writes = vec![
        PlannedWrite::write(
            RepoPath::parse(format!(".vela/policies/{}.json", policy.id))
                .map_err(transaction_error)?,
            WriteClass::CanonicalEvidence,
            active_bytes,
        ),
        PlannedWrite::write(
            RepoPath::parse(format!(".vela/policies/{}.sig.json", policy.id))
                .map_err(transaction_error)?,
            WriteClass::Authority,
            signature_bytes,
        ),
        PlannedWrite::delete(
            RepoPath::parse(".vela/policies/active.sig.json").map_err(transaction_error)?,
            WriteClass::Authority,
        ),
        PlannedWrite::write(
            RepoPath::parse(format!(".vela/policies/revoked-{}.json", policy.id))
                .map_err(transaction_error)?,
            WriteClass::Authority,
            marker_bytes,
        ),
    ];
    let read_set = vec![
        InputBinding::existing_file(
            frontier,
            RepoPath::parse(".vela/policies/active.json").map_err(transaction_error)?,
        )
        .map_err(transaction_error)?,
        InputBinding::existing_file(
            frontier,
            RepoPath::parse(".vela/policies/active.sig.json").map_err(transaction_error)?,
        )
        .map_err(transaction_error)?,
        InputBinding::existing_file(
            frontier,
            RepoPath::parse(".vela/actors.json").map_err(transaction_error)?,
        )
        .map_err(transaction_error)?,
    ];
    let policy_id = policy.id.clone();
    let head = commit_policy_head_transaction(
        frontier,
        barrier,
        &original,
        actor,
        &key,
        reason,
        &revoked_at,
        PolicyHeadAction::Revoke,
        None,
        request_root,
        operation_id,
        writes,
        read_set,
    )?;
    if load_active_policy(frontier)
        .map_err(transaction_error)?
        .is_some()
    {
        return Err(transaction_error(
            "completed revocation still exposes an active policy",
        ));
    }
    Ok((policy_id, head))
}

/// One shadow decision from the dry-run.
struct ShadowRow {
    proposal: String,
    kind: String,
    claim_class: String,
    outcome: Outcome,
    matched_rule_ids: Vec<String>,
    reasons: Vec<String>,
}

/// Dry-run the policy over every pending proposal using the shared review-fact
/// derivation. When retained Receipt v1 bytes are unavailable, that derivation
/// fails closed rather than reconstructing optimistic facts. Pure: never
/// mutates.
fn evaluate_pending(
    project: &Project,
    policy: &AcceptancePolicy,
    now: &str,
    frontier: Option<&Path>,
) -> Vec<ShadowRow> {
    project
        .proposals
        .iter()
        .filter(|p| p.status == "pending_review")
        .map(|p| {
            let receipt = frontier.and_then(|frontier| {
                crate::review_material::frontier_receipt_for_proposal(frontier, p)
            });
            let ctx = crate::review_material::derive_existing_proposal_policy_context(
                project,
                &p.id,
                receipt.as_ref(),
                now,
            );
            let d = evaluate(policy, &ctx, now);
            ShadowRow {
                proposal: p.id.clone(),
                kind: p.kind.clone(),
                claim_class: ctx.claim_class,
                outcome: d.outcome,
                matched_rule_ids: d.matched_rule_ids,
                reasons: d.reasons,
            }
        })
        .collect()
}

/// One policy-lane admission read back out of the event log.
struct Admission {
    policy_id: String,
    event_id: String,
    proposal_id: String,
    rule_ids: Vec<String>,
    timestamp: String,
}

/// Every event the policy lane admitted, in log order: the `policy_lane`
/// payload block is the tamper-evident stamp `policy_accept` writes into
/// the content address, so reading it back requires no join.
fn lane_admissions(project: &Project) -> Vec<Admission> {
    project
        .events
        .iter()
        .filter_map(|ev| {
            let lane = ev.payload.get(POLICY_LANE_PAYLOAD_KEY)?;
            Some(Admission {
                policy_id: lane
                    .get("policy_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                event_id: ev.id.clone(),
                proposal_id: lane
                    .get("certificate")
                    .and_then(|c| c.get("proposal_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                rule_ids: lane
                    .get("rule_ids")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|r| r.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                timestamp: ev.timestamp.clone(),
            })
        })
        .collect()
}

fn policy_has_causal_auto_permit(policy: &AcceptancePolicy) -> bool {
    policy.expires_at == CAUSALLY_UNBOUNDED_POLICY_EXPIRY
}

// ── Rendering ──────────────────────────────────────────────────────────

fn constraints_summary(c: &Constraints) -> String {
    let mut parts = vec![
        format!("A>={}", c.required_assurance_min),
        format!("changed<={}", c.max_changed_findings),
        format!("deps<={}", c.max_downstream_dependents),
    ];
    parts.push(if c.allow_semantic_text_change {
        "text change allowed".to_string()
    } else {
        "no text change".to_string()
    });
    if c.require_independence {
        parts.push("independence".to_string());
    }
    if c.require_method_integrity {
        parts.push("method integrity".to_string());
    }
    if c.allow_contested {
        parts.push("contested allowed".to_string());
    }
    if c.allow_governance_mutation {
        parts.push("governance allowed".to_string());
    }
    parts.join(" · ")
}

fn render_policy(p: &AcceptancePolicy) {
    println!("  policy    {} · epoch {}", p.id, p.epoch);
    if !p.frontier_id.is_empty() {
        println!("  frontier  {}", p.frontier_id);
    }
    if !p.issued_by.is_empty() {
        println!("  issued by {}", p.issued_by.join(", "));
    }
    if policy_has_causal_auto_permit(p) {
        println!(
            "  default   {} · valid until signed rotation or revocation",
            p.default.as_str()
        );
    } else {
        println!(
            "  default   {} · expires {} · Permit remains human-routed",
            p.default.as_str(),
            p.expires_at
        );
    }
    println!();
    println!("  rules");
    for r in &p.rules {
        render_rule(r);
    }
}

fn render_rule(r: &PolicyRule) {
    let classes = if r.claim_classes.is_empty() {
        "any claim class".to_string()
    } else {
        r.claim_classes.join(", ")
    };
    println!("    {:<6} {}  →  {}", r.effect.as_str(), r.id, classes);
    if r.effect == Outcome::Permit {
        println!(
            "           {}",
            style::dim(&constraints_summary(&r.constraints))
        );
    }
}

fn print_admission_rows(rows: &[&Admission]) {
    for a in rows {
        let rules = if a.rule_ids.is_empty() {
            String::new()
        } else {
            format!("  ← {}", a.rule_ids.join(", "))
        };
        println!(
            "    {}  {}  {}{rules}",
            a.timestamp, a.event_id, a.proposal_id
        );
    }
}

fn confirm(prompt: &str) -> bool {
    crate::cli::prompt::confirm(prompt)
}

// ── The six verbs ──────────────────────────────────────────────────────

/// `vela policy show` — the active policy: id, epoch, rules, expiry, its
/// signature state (a sealed-unsigned policy carries NO authority), and what
/// it admitted lately (the last policy-lane events stamped with its id).
pub(crate) fn cmd_policy_show(frontier: &Path, json: bool) {
    let active = active_path(frontier);
    if !active.exists() {
        if json {
            print_json(&json!({
                "ok": true,
                "command": "policy.show",
                "state": "absent",
                "policy": serde_json::Value::Null,
                "signature": serde_json::Value::Null,
                "admissions": { "count": 0, "last": [] },
            }));
        } else {
            ui::header("POLICY", &frontier.display().to_string(), None);
            println!("  no active policy — the lane is closed; every accept is a key ceremony");
            println!(
                "  open one: `vela policy draft <template>` (templates: {TEMPLATES}), then `vela policy sign`"
            );
        }
        return;
    }

    let snapshot = load_active_policy_snapshot(frontier).unwrap_or_else(|e| {
        fail_with(
            ErrorKind::Domain,
            &format!("active policy pair is broken: {e}"),
            Some("inspect the exact active policy bytes; invalid governance never fails open"),
        )
    });
    let policy = read_sealed_active(frontier).unwrap_or_else(|e| e.fail());
    let sig_present = snapshot.signature_bytes.is_some();
    let signed = snapshot.verified.clone();
    let record: Option<PolicySignatureRecord> = if sig_present {
        std::fs::read_to_string(active_sig_path(frontier))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
    } else {
        None
    };
    let revoked = revoked_marker_path(frontier, &policy.id).exists();
    let now = Utc::now().to_rfc3339();
    let expired = policy.is_expired(&now);

    // What it admitted lately. A missing/partial event log is fine — the
    // policy files are still worth showing.
    let admissions: Vec<Admission> = repo::load_from_path(frontier)
        .map(|p| lane_admissions(&p))
        .unwrap_or_default();
    let mine: Vec<&Admission> = admissions
        .iter()
        .filter(|a| a.policy_id == policy.id)
        .collect();
    let last: Vec<&Admission> = mine.iter().rev().take(5).rev().copied().collect();

    let state = if signed.is_some() {
        "signed"
    } else if revoked {
        "revoked"
    } else {
        "sealed_unsigned"
    };

    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.show",
            "state": state,
            "expired": expired,
            "auto_permit_enabled": signed.is_some() && policy_has_causal_auto_permit(&policy),
            "policy": serde_json::to_value(&policy).unwrap_or_default(),
            "signature": record.as_ref().map(|r| json!({
                "signer_pubkey_hex": r.signer_pubkey_hex,
                "signed_at": r.signed_at,
            })),
            "admissions": {
                "count": mine.len(),
                "last": last.iter().map(|a| json!({
                    "event": a.event_id,
                    "proposal": a.proposal_id,
                    "rule_ids": a.rule_ids,
                    "timestamp": a.timestamp,
                })).collect::<Vec<_>>(),
            },
        }));
        return;
    }

    ui::header("POLICY", &frontier.display().to_string(), None);
    render_policy(&policy);
    println!();
    match (&signed, revoked) {
        (Some(v), _) => {
            let signed_at = record.as_ref().map(|r| r.signed_at.as_str()).unwrap_or("");
            if policy_has_causal_auto_permit(&policy) {
                println!(
                    "  {} signed by {}… at {signed_at} — the lane is open",
                    style::ok("state"),
                    &v.signer_pubkey_hex[..16.min(v.signer_pubkey_hex.len())]
                );
            } else {
                println!(
                    "  {} signed by {}… at {signed_at} — Permit stays human-routed",
                    style::ok("state"),
                    &v.signer_pubkey_hex[..16.min(v.signer_pubkey_hex.len())]
                );
            }
        }
        (None, true) => {
            println!(
                "  {} REVOKED — the lane is closed; this policy is never re-signed",
                style::lost("state")
            );
        }
        (None, false) => {
            println!(
                "  {} SEALED-UNSIGNED — carries no authority; sign with `vela policy sign`",
                style::warn("state")
            );
        }
    }
    if expired {
        println!(
            "  {} expired at {} — the evaluator denies everything",
            style::lost("note"),
            policy.expires_at
        );
    }
    println!();
    if mine.is_empty() {
        println!("  admitted under this policy: nothing yet");
    } else {
        println!(
            "  admitted under this policy: {} event(s), last {}:",
            mine.len(),
            last.len()
        );
        print_admission_rows(&last);
    }
}

/// `vela policy suggest` — the histogram of asks plus the covering rules.
/// Pure read: it seals nothing and signs nothing. The next command it
/// hands is `draft` (a template or --from-suggest), then `sign`.
pub(crate) fn cmd_policy_suggest(frontier: &Path, json: bool) {
    let project =
        repo::load_from_path(frontier).unwrap_or_else(|e| fail_with(ErrorKind::Domain, &e, None));
    let rows = super::policy_suggest::ask_histogram(&project, frontier)
        .unwrap_or_else(|e| fail_with(ErrorKind::Domain, &e, None));
    let suggested = super::policy_suggest::suggestions(&rows);
    let has_signed = active_sig_path(frontier).exists();

    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.suggest",
            "asks": rows,
            "suggestions": suggested,
            "threshold": super::policy_suggest::SUGGEST_THRESHOLD,
        }));
        return;
    }

    ui::header("POLICY", &frontier.display().to_string(), Some("suggest"));
    if rows.is_empty() {
        println!("  no asks on record — nothing reached your key that a rule could absorb");
        return;
    }
    println!("  what has been reaching your key:");
    for r in &rows {
        println!(
            "  {:>4}x  {}  {}",
            r.count,
            r.claim_class,
            style::dim(&r.reason.replace('_', " "))
        );
    }
    if suggested.is_empty() {
        println!();
        println!(
            "  nothing recurs past the bar ({}x) — these asks are judgment, not friction",
            super::policy_suggest::SUGGEST_THRESHOLD
        );
        return;
    }
    for s in &suggested {
        println!();
        println!(
            "  {} {} asks were {} — one signature covers the class:",
            style::brass("suggest"),
            s.covers,
            s.claim_class
        );
        render_rule(&s.rule);
        let replace_flag = if has_signed { " --replace" } else { "" };
        match &s.template {
            Some(t) => {
                println!("  next: `vela policy draft {t} .{replace_flag} && vela policy sign .`")
            }
            None => println!(
                "  next: `vela policy draft --from-suggest .{replace_flag} && vela policy sign .`"
            ),
        }
    }
}

/// The one-line nudge other surfaces (the sign session) print. None when
/// nothing crosses the bar.
pub(crate) fn suggest_hint(frontier: &Path) -> Option<String> {
    let project = repo::load_from_path(frontier).ok()?;
    let rows = super::policy_suggest::ask_histogram(&project, frontier).ok()?;
    let top = super::policy_suggest::suggestions(&rows)
        .into_iter()
        .next()?;
    Some(format!(
        "{} of your recent asks were {} — `vela policy suggest` shows the rule that would cover them",
        top.covers, top.claim_class
    ))
}

/// `vela policy draft --from-suggest` — seal the suggested covering
/// rules (still unsigned; authority arrives with `vela policy sign`).
pub(crate) fn cmd_policy_draft_from_suggest(frontier: &Path, replace: bool, json: bool) {
    let project =
        repo::load_from_path(frontier).unwrap_or_else(|e| fail_with(ErrorKind::Domain, &e, None));
    let rows = super::policy_suggest::ask_histogram(&project, frontier)
        .unwrap_or_else(|e| fail_with(ErrorKind::Domain, &e, None));
    let suggested = super::policy_suggest::suggestions(&rows);
    if suggested.is_empty() {
        fail_with(
            ErrorKind::NotFound,
            "nothing to draft — no class recurs past the suggestion bar",
            Some("`vela policy suggest` shows the histogram"),
        );
    }
    let rules: Vec<PolicyRule> = suggested.iter().map(|s| s.rule.clone()).collect();
    let (policy, replaced) = seal_policy(frontier, rules, replace).unwrap_or_else(|e| e.fail());

    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.draft",
            "template": "from-suggest",
            "policy_id": policy.id,
            "epoch": policy.epoch,
            "replaced_signed": replaced,
            "signed": false,
            "policy": serde_json::to_value(&policy).unwrap_or_default(),
            "next": "vela policy sign",
        }));
        return;
    }
    ui::header("POLICY", "from-suggest", Some("sealed draft"));
    println!(
        "  covering rules for {} recurring class(es), sealed from the ask histogram",
        suggested.len()
    );
    println!();
    render_policy(&policy);
    println!();
    if replaced {
        println!(
            "  {} the outgoing signed policy was snapshotted; the lane is CLOSED until you sign",
            style::warn("rotated")
        );
    }
    println!("  sealed — carries no authority yet");
    println!("  review the rules above, then: `vela policy sign` (one confirm, one key read)");
}

/// `vela policy draft <template>` — seal a policy from the template ladder.
/// Sealing fixes the content-addressed id so the exact rules a human reviews
/// are the exact rules the signature covers. No authority until `sign`.
pub(crate) fn cmd_policy_draft(frontier: &Path, template: &str, replace: bool, json: bool) {
    let (policy, replaced) = draft_policy(frontier, template, replace).unwrap_or_else(|e| e.fail());
    let summary = template_policy(template)
        .map(|(_, s)| s)
        .unwrap_or_default();

    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.draft",
            "template": template,
            "policy_id": policy.id,
            "epoch": policy.epoch,
            "frontier_id": policy.frontier_id,
            "expires_at": policy.expires_at,
            "auto_permit_enabled": policy_has_causal_auto_permit(&policy),
            "replaced_signed": replaced,
            "signed": false,
            "policy": serde_json::to_value(&policy).unwrap_or_default(),
            "next": "vela policy sign",
        }));
        return;
    }

    ui::header("POLICY", template, Some("sealed draft"));
    println!("  {summary}");
    println!();
    render_policy(&policy);
    println!();
    if replaced {
        println!(
            "  {} the outgoing signed policy was snapshotted; the lane is CLOSED until you sign",
            style::warn("rotated")
        );
    }
    println!("  sealed — carries no authority yet");
    println!("  review the rules above, then: `vela policy sign` (one confirm, one key read)");
}

/// `vela policy test` — dry-run the active (or still-sealed) policy over
/// every pending proposal using the same retained review facts as the sign
/// queue. Missing receipt or verifier facts remain conservative. Never mutates.
pub(crate) fn cmd_policy_test(frontier: &Path, json: bool) {
    let spin = (!json).then(|| {
        crate::cli::progress::Spinner::start("dry-running the policy over every pending proposal")
    });
    let policy = read_sealed_active(frontier).unwrap_or_else(|e| e.fail());
    let lane_open = matches!(load_active_policy(frontier), Ok(Some(_)));
    let project =
        repo::load_from_path(frontier).unwrap_or_else(|e| fail_with(ErrorKind::Domain, &e, None));
    let now = Utc::now().to_rfc3339();
    let rows = evaluate_pending(&project, &policy, &now, Some(frontier));

    let permit = rows.iter().filter(|r| r.outcome == Outcome::Permit).count();
    let defer = rows.iter().filter(|r| r.outcome == Outcome::Defer).count();
    let deny = rows.iter().filter(|r| r.outcome == Outcome::Deny).count();

    if let Some(s) = spin {
        s.finish("evaluated");
    }
    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.test",
            "mode": "dry_run",
            "policy_id": policy.id,
            "lane_open": lane_open,
            "evaluator": EVALUATOR_VERSION,
            "now": now,
            "summary": { "permit": permit, "defer": defer, "deny": deny, "total": rows.len() },
            "decisions": rows.iter().map(|r| json!({
                "proposal": r.proposal,
                "kind": r.kind,
                "claim_class": r.claim_class,
                "outcome": r.outcome.as_str(),
                "matched_rules": r.matched_rule_ids,
                "reasons": r.reasons,
            })).collect::<Vec<_>>(),
        }));
        return;
    }

    ui::header("POLICY", &policy.id, Some("dry-run"));
    println!(
        "  {} pending proposal(s): {permit} permit, {defer} defer, {deny} deny",
        rows.len()
    );
    println!();
    for r in rows.iter().take(20) {
        let why = if r.outcome == Outcome::Permit {
            r.matched_rule_ids.first().cloned().unwrap_or_default()
        } else {
            r.reasons.first().cloned().unwrap_or_default()
        };
        println!(
            "  [{}] {} ({}, {})  ← {why}",
            r.outcome.as_str(),
            r.proposal,
            r.kind,
            r.claim_class
        );
    }
    if rows.len() > 20 {
        println!("  … {} more (use --json for all)", rows.len() - 20);
    }
    println!();
    println!("  dry-run from retained proposal, receipt, gate, and graph facts — nothing applied.");
    println!(
        "  missing receipt or verifier facts remain conservative and cannot manufacture a permit."
    );
    if !lane_open {
        println!(
            "  {} this policy is not signed — even a permit would land nothing until `vela policy sign`",
            style::warn("note")
        );
    }
}

/// `vela policy evaluate-proposal <frontier> <proposal_id>` — the CI-callable
/// verdict on ONE proposal, for the auto-merge Action. It answers two questions
/// deterministically and read-only: (1) did the signed policy auto-admit this
/// proposal, and does its *recorded* decision still re-derive `Permit` under the
/// persisted policy (the routing-forgery re-check `vela check` also runs); and
/// (2) is the proposal's Sidon bound a genuine beat of the current best at that
/// `n`. The Action merges iff `mergeable` (admitted && permit && is_beat); a
/// non-permit / non-beat / forged accept yields `mergeable: false` → defer to a
/// human. No key, no mutation.
pub(crate) fn cmd_policy_evaluate_proposal(frontier: &Path, proposal_id: &str, json: bool) {
    let project = match repo::load_from_path(frontier) {
        Ok(p) => p,
        Err(e) => fail_with(
            ErrorKind::NotFound,
            &format!("cannot load frontier: {e}"),
            None,
        ),
    };

    // (1) Admission + forgery re-check. `lane_admissions` reads the tamper-evident
    // `policy_lane` stamp; `verify_policy_lane_events` re-derives Permit from the
    // stamped context under the persisted policy. A pending (deferred) proposal
    // has no admission → not mergeable.
    let admission = lane_admissions(&project)
        .into_iter()
        .find(|a| a.proposal_id == proposal_id);
    let lane_errors = verify_policy_lane_events(&project, frontier);
    let (admitted, rule_ids, admit_event, forgery_error) = match &admission {
        Some(a) => {
            let err = lane_errors
                .iter()
                .find(|e| e.starts_with(&a.event_id))
                .cloned();
            (
                err.is_none(),
                a.rule_ids.clone(),
                Some(a.event_id.clone()),
                err,
            )
        }
        None => (false, Vec::new(), None, None),
    };
    let verdict = if admitted { "permit" } else { "defer" };

    // (2) is_beat: parse the Sidon bound from the proposal's claim text and
    // compare to the best accepted bound at the same n (excluding this
    // proposal's own admitted finding). A cell with no prior bound is a first
    // record, which counts as a beat.
    let claim_text = project
        .proposals
        .iter()
        .find(|p| p.id == proposal_id)
        .and_then(|p| {
            // `finding.add` nests the bundle under `finding`; `vela land` and the
            // raw note/caveat shapes put the text at the top. Check both.
            p.payload
                .pointer("/finding/assertion/text")
                .or_else(|| p.payload.pointer("/assertion/text"))
                .or_else(|| p.payload.pointer("/text"))
                .and_then(|t| t.as_str())
                .map(str::to_string)
        });
    let own_finding_id = admit_event.as_ref().and_then(|eid| {
        project
            .events
            .iter()
            .find(|e| &e.id == eid)
            .map(|e| e.target.id.clone())
    });
    let bound = claim_text.as_deref().and_then(parse_sidon_bound);
    let (n_opt, claimed_opt) = match bound {
        Some((n, k)) => (Some(n), Some(k)),
        None => (None, None),
    };
    let current_best =
        n_opt.and_then(|n| best_sidon_bound_for_n(&project, n, own_finding_id.as_deref()));
    let is_beat = match (claimed_opt, current_best) {
        (Some(k), Some(best)) => k > best,
        (Some(_), None) => true, // first record at this n
        _ => false,
    };
    let mergeable = admitted && verdict == "permit" && is_beat;

    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.evaluate-proposal",
            "frontier": frontier.display().to_string(),
            "proposal_id": proposal_id,
            "admitted": admitted,
            "verdict": verdict,
            "rule_ids": rule_ids,
            "n": n_opt,
            "claimed": claimed_opt,
            "current_best": current_best,
            "is_beat": is_beat,
            "forgery_check_error": forgery_error,
            "mergeable": mergeable,
        }));
    } else {
        println!("policy · evaluate-proposal · {proposal_id}");
        let rule = if rule_ids.is_empty() {
            String::new()
        } else {
            format!(", rule {}", rule_ids.join(","))
        };
        println!("  admitted:  {admitted}  (verdict: {verdict}{rule})");
        match (n_opt, claimed_opt, current_best) {
            (Some(n), Some(k), Some(b)) => println!(
                "  beat:      a({n}) >= {k} vs current {b} -> {}",
                if is_beat { "BEATS" } else { "not a beat" }
            ),
            (Some(n), Some(k), None) => println!("  beat:      a({n}) >= {k} -> FIRST RECORD"),
            _ => println!("  beat:      (not a parseable Sidon bound)"),
        }
        if let Some(err) = &forgery_error {
            println!("  {} forgery re-check: {err}", style::warn("!"));
        }
        println!("  mergeable: {mergeable}");
    }
}

/// `vela ci verdict --base <ref>` — the whole auto-merge decision in one verb, so
/// a frontier's GitHub Action is ~15 lines. Discovers the proposals a PR adds
/// (diffed against `<base>`), re-derives each one's exact-lane `machine_verified`
/// verdict from the frozen floor, confirms at least one is a genuine beat of the
/// accepted record, and guards that the PR only touches the append-only store +
/// its derived views (never `bounds.json` or the pinned `vela_version`). Exits 0
/// iff the PR may auto-merge. The floor (replay, signatures, reproduce, hash
/// parity) is the shared `vela-science/vela` action's job, run before this.
pub(crate) fn cmd_ci_verdict(frontier: &Path, base: &str, json: bool) {
    use std::process::Command;

    let git = |args: &[&str]| -> Result<String, String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(frontier)
            .args(args)
            .output()
            .map_err(|e| format!("git: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };

    // The frontier's path prefix within the repo ("" at the root), so root-relative
    // diff paths can be compared against the store layout.
    let prefix = git(&["rev-parse", "--show-prefix"]).unwrap_or_default();
    let rel = |p: &str| -> String { p.strip_prefix(&prefix).unwrap_or(p).to_string() };

    if git(&["rev-parse", "--verify", "--quiet", base]).is_err() {
        fail_with(
            ErrorKind::NotFound,
            &format!("base ref '{base}' not found"),
            Some("CI must fetch it — actions/checkout with fetch-depth: 0"),
        );
    }

    let mut reasons: Vec<String> = Vec::new();

    // (1) ALLOWLIST. A producer PR may add to the append-only store and its
    // derived views; anything else (bounds.json, .github, docs) waits for a human.
    let changed = git(&["diff", "--name-only", &format!("{base}...HEAD")]).unwrap_or_default();
    for path in changed.lines().filter(|l| !l.is_empty()) {
        let p = rel(path);
        let allowed = p.starts_with(".vela/")
            || p.starts_with("witnesses/")
            || p.starts_with("records/")
            || p.starts_with("proof/")
            || p == "frontier.json"
            || p == "frontier.yaml"
            || p == "vela.lock";
        if p == "bounds.json" {
            reasons.push("bounds.json changed (the beat oracle is not producer-editable)".into());
        } else if !allowed {
            reasons.push(format!("changed outside the store: {p}"));
        }
    }

    // The pinned verifier must not move under a producer PR (downgrade guard).
    let ver_of = |lock: &str| -> Option<String> {
        lock.lines().find_map(|l| {
            l.trim()
                .strip_prefix("vela_version:")
                .map(|v| v.trim().to_string())
        })
    };
    let base_ver = git(&["show", &format!("{base}:{prefix}vela.lock")])
        .ok()
        .and_then(|s| ver_of(&s));
    let head_ver = std::fs::read_to_string(frontier.join("vela.lock"))
        .ok()
        .and_then(|s| ver_of(&s));
    if let (Some(b), Some(h)) = (&base_ver, &head_ver)
        && b != h
    {
        reasons.push(format!("vela_version changed ({b} -> {h})"));
    }

    // (2) NEW PROPOSALS = present at HEAD, absent at base.
    let base_props: std::collections::HashSet<String> = git(&[
        "ls-tree",
        "--name-only",
        "-r",
        base,
        "--",
        &format!("{prefix}.vela/proposals"),
    ])
    .unwrap_or_default()
    .lines()
    .filter_map(|p| {
        std::path::Path::new(p)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
    })
    .collect();

    let project = repo::load_from_path(frontier)
        .unwrap_or_else(|e| fail_with(ErrorKind::NotFound, &format!("load frontier: {e}"), None));
    let new_proposals: Vec<&StateProposal> = project
        .proposals
        .iter()
        .filter(|p| p.kind == "finding.add" && !base_props.contains(&p.id))
        .collect();

    // (3) Per new proposal: exact-lane machine_verified + is-it-a-beat.
    let mut all_mv = true;
    let mut any_beat = false;
    let mut per: Vec<serde_json::Value> = Vec::new();
    for p in &new_proposals {
        let vf = p.target.id.clone();
        let verdict = match crate::cli_engine::evaluate_exact_policy_route(frontier, &vf) {
            Ok(v) => v,
            Err(e) => {
                all_mv = false;
                per.push(
                    json!({"proposal": p.id, "finding": vf, "machine_verified": false, "error": e}),
                );
                continue;
            }
        };
        let mv = verdict.would_permit;
        let claim = verdict.canonical_claim.clone().or_else(|| {
            p.payload
                .pointer("/finding/assertion/text")
                .or_else(|| p.payload.pointer("/assertion/text"))
                .and_then(|t| t.as_str())
                .map(str::to_string)
        });
        let bound = claim.as_deref().and_then(parse_sidon_bound);
        let (n_opt, claimed_opt) = bound.map_or((None, None), |(n, k)| (Some(n), Some(k)));
        let best = n_opt.and_then(|n| best_sidon_bound_for_n(&project, n, Some(&vf)));
        let is_beat = match (claimed_opt, best) {
            (Some(k), Some(b)) => k > b,
            (Some(_), None) => true,
            _ => false,
        };
        if !mv {
            all_mv = false;
        }
        if is_beat {
            any_beat = true;
        }
        per.push(json!({
            "proposal": p.id, "finding": vf,
            "machine_verified": mv, "is_beat": is_beat,
            "claim": claim, "n": n_opt, "claimed": claimed_opt, "current_best": best,
        }));
    }

    if new_proposals.is_empty() {
        reasons.push("no new finding proposals in this PR".into());
    }
    if !all_mv {
        reasons.push("not every new finding is machine_verified".into());
    }
    if !new_proposals.is_empty() && !any_beat {
        reasons.push("no new finding beats the accepted record".into());
    }

    let admit = reasons.is_empty();

    if json {
        print_json(&json!({
            "ok": true,
            "command": "ci.verdict",
            "frontier": frontier.display().to_string(),
            "base": base,
            "admit": admit,
            "machine_verified": all_mv && !new_proposals.is_empty(),
            "is_beat": any_beat,
            "new_proposals": new_proposals.iter().map(|p| p.id.clone()).collect::<Vec<_>>(),
            "per_proposal": per,
            "reasons": reasons,
        }));
    } else {
        println!("ci · verdict · base {base}");
        for pp in &per {
            println!(
                "  {} machine_verified={} beat={}",
                pp.get("finding").and_then(|v| v.as_str()).unwrap_or("?"),
                pp.get("machine_verified")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                pp.get("is_beat").and_then(|v| v.as_bool()).unwrap_or(false),
            );
        }
        if admit {
            println!("  => ADMIT: a gate-clean machine_verified beat");
        } else {
            println!("  => HOLD (needs a human):");
            for r in &reasons {
                println!("     - {r}");
            }
        }
    }
    if !admit {
        std::process::exit(1);
    }
}

/// Parse the canonical Sidon claim text `… a(N) >= K …` into `(N, K)`. Matches
/// the format `submit.py` emits; returns `None` for anything else.
fn parse_sidon_bound(text: &str) -> Option<(i64, i64)> {
    let idx = text.find("a(")?;
    let rest = &text[idx + 2..];
    let close = rest.find(')')?;
    let n: i64 = rest[..close].trim().parse().ok()?;
    let after = &rest[close + 1..];
    let ge = after.find(">=")?;
    let tail = &after[ge + 2..];
    let digits: String = tail
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    Some((n, digits.parse().ok()?))
}

/// The best accepted Sidon lower bound at dimension `n`, ignoring one finding
/// (the proposal's own admitted finding, so a beat is measured against the rest).
pub(crate) fn best_sidon_bound_for_n(
    project: &Project,
    n: i64,
    exclude_finding: Option<&str>,
) -> Option<i64> {
    let mut best: Option<i64> = None;
    for f in &project.findings {
        if Some(f.id.as_str()) == exclude_finding {
            continue;
        }
        if let Some((fn_, fk)) = parse_sidon_bound(&f.assertion.text)
            && fn_ == n
        {
            best = Some(best.map_or(fk, |b| b.max(fk)));
        }
    }
    best
}

/// `vela policy sign` — THE ceremony. A human (never `agent:`/`ci:`) reviews
/// the sealed rules, confirms once, the key is read once, and the signature
/// over the canonical bytes opens the lane: every matching permit rule is now
/// live authority with no per-item key ceremony.
pub(crate) fn cmd_policy_sign(frontier: &Path, key: Option<&Path>, yes: bool, json: bool) {
    crate::cli::sign_session::ceremony_binary_gate(!yes);
    // Humans only: the whole point of the lane is that a HUMAN signed once.
    let actor = crate::cli_identity::resolve_decision_actor(None);
    let policy = read_sealed_active(frontier).unwrap_or_else(|e| e.fail());
    if revoked_marker_path(frontier, &policy.id).exists() {
        fail_with(
            ErrorKind::Custody,
            &format!(
                "{} was revoked — a revoked policy is never re-signed",
                policy.id
            ),
            Some("draft a new epoch: `vela policy draft <template>`"),
        );
    }
    if let Err(e) = load_active_policy(frontier)
        && !json
    {
        println!(
            "  {} existing signature is broken ({e}) — re-signing replaces it",
            style::warn("note")
        )
    }

    if !json {
        ui::header("POLICY", &policy.id, Some("sign — the lane opens"));
        render_policy(&policy);
        println!();
        println!("  signing as {actor}");
        if policy_has_causal_auto_permit(&policy) {
            println!("  a signature makes every permit rule above LIVE: agents land that class of");
            println!("  gated work until a signed replacement or `policy revoke` closes it.");
        } else {
            println!("  this finite wall-clock policy can route Defer/Deny, but Permit remains");
            println!(
                "  human-routed because an unsigned event cannot prove it occurred before expiry."
            );
        }
        println!();
    }
    if !yes {
        ui::ensure_can_prompt("policy sign", "pass --yes to sign non-interactively");
        if !confirm(&format!("  sign {} and open the lane? [y/N] ", policy.id)) {
            fail_with(
                ErrorKind::Usage,
                "not signed — no confirmation",
                Some("re-run and answer y, or pass --yes"),
            );
        }
    }

    let (policy, record, head) = sign_active_policy_transactional(
        frontier,
        &actor,
        &policy.id,
        || Utc::now().to_rfc3339(),
        || crate::cli_identity::resolve_signing_key(key),
    )
    .unwrap_or_else(|e| e.fail());

    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "command": "policy",
                "policy_id": policy.id,
                "epoch": policy.epoch,
                "signer": record.signer_pubkey_hex,
                "policy_head_event_id": head.event_id,
                "policy_head_epoch": head.epoch,
                "auto_permit_enabled": policy_has_causal_auto_permit(&policy),
            })
        );
        return;
    }

    println!();
    if policy_has_causal_auto_permit(&policy) {
        println!("  {} policy live — the lane is open", style::ok("signed"));
    } else {
        println!(
            "  {} policy signed — finite-window Permit remains human-routed",
            style::ok("signed")
        );
    }
    println!(
        "  {} · epoch {} · signer {}…",
        policy.id,
        policy.epoch,
        &record.signer_pubkey_hex[..16.min(record.signer_pubkey_hex.len())]
    );
    println!();
    if policy_has_causal_auto_permit(&policy) {
        for r in policy.rules.iter().filter(|r| r.effect == Outcome::Permit) {
            println!(
                "  auto-lands: {}  ({})",
                r.claim_classes.join(", "),
                constraints_summary(&r.constraints)
            );
        }
    }
    println!("  everything else defers to `vela sign` — the queue, not silence.");
    println!("  close the lane anytime: `vela policy revoke --reason <why>`");
}

/// `vela policy revoke --reason <why>` — a human signs a causal Revoke head.
/// The same recoverable transaction deletes `active.sig.json` while retaining
/// content-addressed snapshots, so old admissions keep verifying and no
/// pointer-only state can contradict the signed close.
pub(crate) fn cmd_policy_revoke(
    frontier: &Path,
    key: Option<&Path>,
    reason: &str,
    yes: bool,
    json: bool,
) {
    crate::cli::sign_session::ceremony_binary_gate(!yes);
    let actor = crate::cli_identity::resolve_decision_actor(None);
    if reason.trim().is_empty() {
        fail_with(
            ErrorKind::Usage,
            "a revocation needs a reason — it is recorded next to the closed lane",
            Some("vela policy revoke --reason \"rotating to a tighter epoch\""),
        );
    }
    let policy = read_sealed_active(frontier).unwrap_or_else(|e| e.fail());
    let admitted = repo::load_from_path(frontier)
        .map(|p| {
            lane_admissions(&p)
                .iter()
                .filter(|a| a.policy_id == policy.id)
                .count()
        })
        .unwrap_or(0);

    if !json {
        ui::header("POLICY", &policy.id, Some("revoke — closing the lane"));
        println!("  epoch {} · admitted {admitted} event(s)", policy.epoch);
        println!("  reason: {reason}");
        println!("  snapshots stay under .vela/policies/ — past admissions keep verifying.");
        println!();
    }
    if !yes {
        ui::ensure_can_prompt("policy revoke", "pass --yes to revoke non-interactively");
        if !confirm(&format!("  close the lane for {}? [y/N] ", policy.id)) {
            fail_with(
                ErrorKind::Usage,
                "not revoked — no confirmation",
                Some("re-run and answer y, or pass --yes"),
            );
        }
    }

    let (vap, head) = revoke_active_policy_transactional(
        frontier,
        &actor,
        &policy.id,
        || Utc::now().to_rfc3339(),
        || crate::cli_identity::resolve_signing_key(key),
        reason,
    )
    .unwrap_or_else(|e| e.fail());
    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "command": "policy",
                "policy_id": vap,
                "revoked": true,
                "reason": reason,
                "policy_head_event_id": head.event_id,
                "policy_head_epoch": head.epoch,
            })
        );
        return;
    }
    println!();
    println!(
        "  {} lane closed — {vap} no longer admits anything",
        style::ok("revoked")
    );
    println!("  reopen with a NEW policy: `vela policy draft <template>` then `vela policy sign`");
}

/// `vela policy log` — every policy-lane admission across ALL policies (the
/// active one and every rotated-out epoch), grouped by policy id.
pub(crate) fn cmd_policy_log(frontier: &Path, json: bool) {
    let project =
        repo::load_from_path(frontier).unwrap_or_else(|e| fail_with(ErrorKind::Domain, &e, None));
    let admissions = lane_admissions(&project);
    let mut by_policy: std::collections::BTreeMap<String, Vec<&Admission>> =
        std::collections::BTreeMap::new();
    for a in &admissions {
        by_policy.entry(a.policy_id.clone()).or_default().push(a);
    }
    let active_id = read_sealed_active(frontier).ok().map(|p| p.id);

    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.log",
            "total": admissions.len(),
            "policies": by_policy.iter().map(|(pid, rows)| json!({
                "policy_id": pid,
                "active": Some(pid) == active_id.as_ref(),
                "count": rows.len(),
                "admissions": rows.iter().map(|a| json!({
                    "event": a.event_id,
                    "proposal": a.proposal_id,
                    "rule_ids": a.rule_ids,
                    "timestamp": a.timestamp,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }));
        return;
    }

    ui::header(
        "POLICY",
        &frontier.display().to_string(),
        Some("admissions"),
    );
    if by_policy.is_empty() {
        println!("  no policy-lane admissions yet — every event in this log is key-signed");
        println!("  open a lane: `vela policy draft <template>` then `vela policy sign`");
        return;
    }
    println!(
        "  {} admission(s) under {} policy(ies)",
        admissions.len(),
        by_policy.len()
    );
    for (pid, rows) in &by_policy {
        let live = if Some(pid) == active_id.as_ref() {
            format!("  {}", style::live("active"))
        } else {
            String::new()
        };
        println!();
        println!("  {pid} — {} admission(s){live}", rows.len());
        print_admission_rows(rows);
    }
}

// ── Transitional argv shim ─────────────────────────────────────────────

/// TRANSITIONAL: the pre-clap `vela policy` intercept in
/// `cli/mod.rs::run_from_args` still routes raw argv here. The clap surface
/// (`Commands::Policy` → the typed `cmd_policy_*` functions above) is the
/// real dispatch; once the lead deletes that intercept, delete this shim —
/// it exists only so both paths speak the same verbs meanwhile.
pub(crate) fn run(args: &[String]) {
    let verb = args.get(2).map(String::as_str).unwrap_or("");
    let json = args.iter().any(|a| a == "--json");
    let replace = args.iter().any(|a| a == "--replace");
    let from_suggest = args.iter().any(|a| a == "--from-suggest");
    let yes = args.iter().any(|a| a == "--yes");
    let value_of = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let key = value_of("--key").map(PathBuf::from);
    let reason = value_of("--reason");
    let actor = value_of("--as");

    // Positional operands after the verb, skipping flags and their values.
    let mut positionals: Vec<String> = Vec::new();
    let mut i = 3;
    while i < args.len() {
        let a = &args[i];
        if a == "--key" || a == "--reason" || a == "--as" {
            i += 2;
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        positionals.push(a.clone());
        i += 1;
    }

    // `policy` is intercepted before clap, so its help is hand-rolled; fold
    // in the same EXAMPLES block the clap verbs carry (Phase 1: both surfaces).
    let usage = format!(
        "usage: vela policy <show|suggest|draft <template>|test|evaluate-proposal <vpr_>|\
                 sign|revoke --reason <why>|retire-legacy --reason <why> --as <actor>|log> \
                 [frontier] [--json] [--replace] [--from-suggest] [--yes] [--key <path>]\n\n{}",
        crate::cli::help_text::POLICY
    );
    match verb {
        "evaluate-proposal" => {
            ui::set_mode("policy", json);
            // Operands are `<vpr_id>` and an optional frontier, order-free: the
            // vpr_ token is the proposal, the other positional is the frontier.
            let pid = positionals
                .iter()
                .find(|p| p.starts_with("vpr_"))
                .cloned()
                .unwrap_or_else(|| {
                    fail_with(
                        ErrorKind::Usage,
                        "evaluate-proposal needs a proposal id (vpr_…)",
                        Some("vela policy evaluate-proposal . vpr_… --json"),
                    )
                });
            let dir = ui::resolve_frontier(
                positionals
                    .iter()
                    .find(|p| !p.starts_with("vpr_"))
                    .map(PathBuf::from),
            );
            cmd_policy_evaluate_proposal(&dir, &pid, json);
        }
        "suggest" => {
            ui::set_mode("policy", json);
            let dir = ui::resolve_frontier(positionals.first().map(PathBuf::from));
            cmd_policy_suggest(&dir, json);
        }
        "retire-legacy" => {
            ui::set_mode("policy", json);
            let custody_flag_present = args
                .iter()
                .any(|arg| arg == "--key" || arg.starts_with("--key="));
            if custody_flag_present || yes {
                fail_with(
                    ErrorKind::Usage,
                    "retire-legacy is prepare-only and does not accept --key or --yes",
                    Some(
                        "prepare the proposal, then use the existing isolated `vela sign` ceremony",
                    ),
                );
            }
            let reason = reason.unwrap_or_else(|| {
                fail_with(
                    ErrorKind::Usage,
                    "retire-legacy needs --reason <why>",
                    Some(
                        "vela policy retire-legacy . --reason \"retire unsupported prelaunch bytes\" --as agent:<you>",
                    ),
                )
            });
            let actor = actor.unwrap_or_else(|| {
                fail_with(
                    ErrorKind::Usage,
                    "retire-legacy needs --as <stable actor>",
                    Some(
                        "vela policy retire-legacy . --reason \"retire unsupported prelaunch bytes\" --as agent:<you>",
                    ),
                )
            });
            let dir = ui::resolve_frontier(positionals.first().map(PathBuf::from));
            crate::config::policy_legacy_retirement::cmd_policy_retire_legacy(
                &dir, &reason, &actor, json,
            );
        }
        "show" | "test" | "log" | "sign" | "revoke" => {
            let interactive = matches!(verb, "sign" | "revoke");
            ui::set_mode("policy", json);
            // JSON is non-interactive (clig.dev): a signing/revoking verb
            // under --json must carry --yes, or it would try to prompt into
            // a stream that must stay pure.
            if json && interactive && !yes {
                fail_with(
                    ErrorKind::Usage,
                    &format!("policy {verb} --json requires --yes (JSON mode is non-interactive)"),
                    Some(&format!("vela policy {verb} … --yes --json")),
                );
            }
            let dir = ui::resolve_frontier(positionals.first().map(PathBuf::from));
            match verb {
                "show" => cmd_policy_show(&dir, json),
                "test" => cmd_policy_test(&dir, json),
                "log" => cmd_policy_log(&dir, json),
                "sign" => cmd_policy_sign(&dir, key.as_deref(), yes, json),
                "revoke" => {
                    let reason = reason.unwrap_or_else(|| {
                        fail_with(
                            ErrorKind::Usage,
                            "revoke needs --reason <why>",
                            Some("vela policy revoke --reason \"rotating epochs\""),
                        )
                    });
                    cmd_policy_revoke(&dir, key.as_deref(), &reason, yes, json);
                }
                _ => unreachable!(),
            }
        }
        "draft" => {
            ui::set_mode("policy", json);
            if from_suggest {
                let dir = ui::resolve_frontier(positionals.first().map(PathBuf::from));
                cmd_policy_draft_from_suggest(&dir, replace, json);
                return;
            }
            let template = positionals.first().cloned().unwrap_or_else(|| {
                fail_with(
                    ErrorKind::Usage,
                    &format!("draft needs a template (templates: {TEMPLATES}) or --from-suggest"),
                    Some("vela policy draft witness-rederivation"),
                )
            });
            let dir = ui::resolve_frontier(positionals.get(1).map(PathBuf::from));
            cmd_policy_draft(&dir, &template, replace, json);
        }
        _ => {
            ui::set_mode("policy", json);
            fail_with(ErrorKind::Usage, &usage, None);
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;
    use vela_protocol::events::StateTarget;
    use vela_protocol::proposals::new_proposal;

    #[test]
    fn parse_sidon_bound_reads_the_canonical_claim() {
        // The exact text submit.py emits.
        assert_eq!(
            parse_sidon_bound(
                "OEIS A309370 a(6) >= 15: a Sidon set of 15 distinct binary vectors …"
            ),
            Some((6, 15))
        );
        assert_eq!(parse_sidon_bound("a(24) >= 1010"), Some((24, 1010)));
        // Non-Sidon / malformed → None (the evaluator then reports not-a-beat).
        assert_eq!(parse_sidon_bound("Lean theorem foo is proved"), None);
        assert_eq!(parse_sidon_bound("a(6) = 15"), None);
        assert_eq!(parse_sidon_bound("a() >= 5"), None);
    }

    #[test]
    fn best_sidon_bound_empty_frontier_is_a_first_record() {
        // The max/exclude behaviour over real findings is covered by the live
        // integration walk; here we pin the base case: no bound at n is None
        // (a first record, which the evaluator scores as a beat).
        let project = vela_protocol::project::assemble("s", vec![], 0, 0, "t");
        assert_eq!(best_sidon_bound_for_n(&project, 13, None), None);
    }

    fn init_frontier(tmp: &TempDir) -> PathBuf {
        let dir = tmp.path().to_path_buf();
        vela_protocol::frontier_repo::initialize(
            &dir,
            vela_protocol::frontier_repo::InitOptions {
                name: "policy-porcelain-test",
                initialize_git: false,
            },
        )
        .unwrap();
        dir
    }

    fn throwaway_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn register_transaction_reviewer(dir: &Path) {
        let key = throwaway_key();
        let mut project = repo::load_from_path(dir).unwrap();
        project.actors.push(vela_protocol::sign::ActorRecord {
            id: "reviewer:test".to_string(),
            public_key: hex::encode(key.verifying_key().to_bytes()),
            algorithm: "ed25519".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        repo::save_to_path(dir, &project).unwrap();
    }

    fn transactionally_sign_policy(dir: &Path, policy_id: &str, signed_at: &str) -> PolicyHead {
        sign_active_policy_transactional(
            dir,
            "reviewer:test",
            policy_id,
            || signed_at.to_string(),
            throwaway_key,
        )
        .unwrap()
        .2
    }

    const AT: &str = "2026-07-03T00:00:00Z";

    #[test]
    fn draft_seals_a_content_addressed_policy() {
        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        let (policy, replaced) = draft_policy(&dir, "witness-rederivation", false).unwrap();
        assert!(!replaced);
        assert!(policy.id.starts_with("vap_"));
        assert!(policy.id_is_valid());
        assert_eq!(policy.epoch, 1);
        assert_eq!(policy.default, Outcome::Defer);
        assert_eq!(policy.expires_at, CAUSALLY_UNBOUNDED_POLICY_EXPIRY);
        assert!(policy_has_causal_auto_permit(&policy));
        // The sealed file on disk re-derives its own id.
        let raw = std::fs::read_to_string(active_path(&dir)).unwrap();
        let on_disk: AcceptancePolicy = serde_json::from_str(&raw).unwrap();
        assert_eq!(on_disk.id, policy.id);
        assert!(on_disk.id_is_valid());
        // Sealed-but-unsigned carries no authority: the loader sees no lane.
        assert!(load_active_policy(&dir).unwrap().is_none());
    }

    #[test]
    fn templates_encode_the_ladder() {
        let (w, _) = template_policy("witness-rederivation").unwrap();
        assert_eq!(w[0].constraints.required_assurance_min, 3);
        assert!(!w[0].constraints.allow_semantic_text_change);
        assert!(w[0].constraints.require_independence);
        assert!(w[0].constraints.require_method_integrity);

        let (s, _) = template_policy("statement-drafts").unwrap();
        assert_eq!(s[0].constraints.required_assurance_min, 2);
        assert!(
            s[0].constraints.allow_semantic_text_change,
            "drafts ARE text"
        );
        assert_eq!(s[0].constraints.max_changed_findings, 1);

        let (n, _) = template_policy("notes-threshold").unwrap();
        assert_eq!(n[0].constraints.required_assurance_min, 0);
        assert_eq!(n[0].constraints.max_changed_findings, 1);
        assert_eq!(n[0].constraints.max_downstream_dependents, 0);

        let (l, _) = template_policy("lean-rederivation").unwrap();
        assert_eq!(l[0].claim_classes, vec!["receipt_lean_kernel_clean"]);
        assert_eq!(l[0].constraints.required_assurance_min, 2);
        assert!(
            l[0].constraints.require_method_integrity,
            "kernel-clean is the floor this lane stands on"
        );
        assert_eq!(l[0].constraints.max_downstream_dependents, 0);

        assert!(template_policy("nonsense").is_none());
    }

    #[test]
    fn draft_refuses_to_overwrite_a_signed_policy() {
        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        let (old, _) = draft_policy(&dir, "witness-rederivation", false).unwrap();
        sign_active_policy(&dir, &throwaway_key(), AT).unwrap();

        let err = draft_policy(&dir, "notes-threshold", false).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Exists, "{}", err.message);

        // --replace rotates: snapshots the outgoing pair, closes the lane.
        let (newer, replaced) = draft_policy(&dir, "notes-threshold", true).unwrap();
        assert!(replaced);
        assert_eq!(newer.epoch, 2);
        assert_ne!(newer.id, old.id);
        assert!(
            load_active_policy(&dir).unwrap().is_none(),
            "lane closes until the new draft is signed"
        );
        assert!(policies_dir(&dir).join(format!("{}.json", old.id)).exists());
        assert!(
            policies_dir(&dir)
                .join(format!("{}.sig.json", old.id))
                .exists()
        );
    }

    #[test]
    fn sign_round_trip_opens_the_lane() {
        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        draft_policy(&dir, "witness-rederivation", false).unwrap();

        let key = throwaway_key();
        let (policy, record) = sign_active_policy(&dir, &key, AT).unwrap();
        assert_eq!(record.policy_id, policy.id);
        assert_eq!(record.signed_at, AT);

        // The exact loader the accept lane uses sees an open lane.
        let verified = load_active_policy(&dir).unwrap().expect("lane open");
        assert_eq!(verified.policy.id, policy.id);
        assert_eq!(verified.signer_pubkey_hex, record.signer_pubkey_hex);
        // Content-addressed snapshots survive future rotation.
        assert!(
            policies_dir(&dir)
                .join(format!("{}.json", policy.id))
                .exists()
        );
        assert!(
            policies_dir(&dir)
                .join(format!("{}.sig.json", policy.id))
                .exists()
        );

        // Idempotent: signing an already-open lane is `Exists`, not a re-sign.
        let err = sign_active_policy(&dir, &key, AT).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Exists);
    }

    #[test]
    fn revoke_closes_the_lane_and_keeps_snapshots() {
        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        draft_policy(&dir, "witness-rederivation", false).unwrap();
        let key = throwaway_key();
        let (policy, _) = sign_active_policy(&dir, &key, AT).unwrap();

        let vap = revoke_active_policy(&dir, "reviewer:test", "rotating the ladder", AT).unwrap();
        assert_eq!(vap, policy.id);
        // The lane is closed: the loader sees nothing to grant authority.
        assert!(load_active_policy(&dir).unwrap().is_none());
        // But the snapshots stay for replay verification of past events.
        assert!(policies_dir(&dir).join(format!("{vap}.json")).exists());
        assert!(policies_dir(&dir).join(format!("{vap}.sig.json")).exists());
        let marker = revoked_marker_path(&dir, &vap);
        assert!(marker.exists());
        let record: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&marker).unwrap()).unwrap();
        assert_eq!(record["reason"], "rotating the ladder");
        assert_eq!(record["revoked_by"], "reviewer:test");

        // A revoked policy is never re-signed.
        let err = sign_active_policy(&dir, &key, AT).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Custody);

        // Revoking again is the idempotent no-op, not a crash.
        let err = revoke_active_policy(&dir, "reviewer:test", "again", AT).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Exists);
    }

    #[test]
    fn transactional_policy_ceremony_activates_revokes_and_reopens_once_per_key_load() {
        use std::cell::Cell;

        const ACTIVATE_AT: &str = "2099-01-01T00:00:00Z";
        const REVOKE_AT: &str = "2099-01-01T00:00:01Z";
        const REOPEN_AT: &str = "2099-01-01T00:00:02Z";

        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        register_transaction_reviewer(&dir);
        let (first_policy, _) = draft_policy(&dir, "witness-rederivation", false).unwrap();

        let drift_clock = Cell::new(0);
        let drift_key_loads = Cell::new(0);
        let drift = sign_active_policy_transactional(
            &dir,
            "reviewer:test",
            "vap_displayed_before_swap",
            || {
                drift_clock.set(drift_clock.get() + 1);
                ACTIVATE_AT.to_string()
            },
            || {
                drift_key_loads.set(drift_key_loads.get() + 1);
                throwaway_key()
            },
        )
        .unwrap_err();
        assert!(
            drift.message.contains("displayed policy"),
            "{}",
            drift.message
        );
        assert_eq!(drift_clock.get(), 0);
        assert_eq!(drift_key_loads.get(), 0);
        assert!(
            current_policy_head(&repo::load_from_path(&dir).unwrap())
                .unwrap()
                .is_none()
        );

        let sign_loads = Cell::new(0);
        let (_, _, activated) = sign_active_policy_transactional(
            &dir,
            "reviewer:test",
            &first_policy.id,
            || ACTIVATE_AT.to_string(),
            || {
                sign_loads.set(sign_loads.get() + 1);
                throwaway_key()
            },
        )
        .unwrap();
        assert_eq!(sign_loads.get(), 1);
        assert_eq!(activated.action, PolicyHeadAction::Activate);
        assert_eq!(
            activated.policy_id.as_deref(),
            Some(first_policy.id.as_str())
        );
        assert_eq!(activated.epoch, 1);
        let loaded = repo::load_from_path(&dir).unwrap();
        let activate_event = loaded
            .events
            .iter()
            .find(|event| event.id == activated.event_id)
            .unwrap();
        assert!(activate_event.signature.is_some());
        assert!(
            vela_protocol::sign::verify_event_signature(
                activate_event,
                &hex::encode(throwaway_key().verifying_key().to_bytes())
            )
            .unwrap()
        );
        let retry_clock = Cell::new(0);
        let retry_key_loads = Cell::new(0);
        let (_, _, activated_retry) = sign_active_policy_transactional(
            &dir,
            "reviewer:test",
            &first_policy.id,
            || {
                retry_clock.set(retry_clock.get() + 1);
                "never sampled".to_string()
            },
            || {
                retry_key_loads.set(retry_key_loads.get() + 1);
                throwaway_key()
            },
        )
        .unwrap();
        assert_eq!(activated_retry, activated);
        assert_eq!(retry_clock.get(), 0);
        assert_eq!(retry_key_loads.get(), 0);

        let revoke_drift_clock = Cell::new(0);
        let revoke_drift_key_loads = Cell::new(0);
        let drift = revoke_active_policy_transactional(
            &dir,
            "reviewer:test",
            "vap_displayed_before_revoke_swap",
            || {
                revoke_drift_clock.set(revoke_drift_clock.get() + 1);
                REVOKE_AT.to_string()
            },
            || {
                revoke_drift_key_loads.set(revoke_drift_key_loads.get() + 1);
                throwaway_key()
            },
            "close the lane",
        )
        .unwrap_err();
        assert!(
            drift.message.contains("displayed policy"),
            "{}",
            drift.message
        );
        assert_eq!(revoke_drift_clock.get(), 0);
        assert_eq!(revoke_drift_key_loads.get(), 0);
        assert_eq!(
            current_policy_head(&repo::load_from_path(&dir).unwrap())
                .unwrap()
                .unwrap(),
            activated
        );

        let revoke_loads = Cell::new(0);
        let (revoked_id, revoked) = revoke_active_policy_transactional(
            &dir,
            "reviewer:test",
            &first_policy.id,
            || REVOKE_AT.to_string(),
            || {
                revoke_loads.set(revoke_loads.get() + 1);
                throwaway_key()
            },
            "close the lane",
        )
        .unwrap();
        assert_eq!(revoke_loads.get(), 1);
        assert_eq!(revoked_id, first_policy.id);
        assert_eq!(revoked.action, PolicyHeadAction::Revoke);
        assert_eq!(revoked.epoch, 2);
        assert!(load_active_policy(&dir).unwrap().is_none());
        let retry_clock = Cell::new(0);
        let retry_key_loads = Cell::new(0);
        let (retried_id, revoked_retry) = revoke_active_policy_transactional(
            &dir,
            "reviewer:test",
            &first_policy.id,
            || {
                retry_clock.set(retry_clock.get() + 1);
                "never sampled".to_string()
            },
            || {
                retry_key_loads.set(retry_key_loads.get() + 1);
                throwaway_key()
            },
            "close the lane",
        )
        .unwrap();
        assert_eq!(retried_id, revoked_id);
        assert_eq!(revoked_retry, revoked);
        assert_eq!(retry_clock.get(), 0);
        assert_eq!(retry_key_loads.get(), 0);

        let resurrect_clock = Cell::new(0);
        let resurrect_key_loads = Cell::new(0);
        let resurrection = sign_active_policy_transactional(
            &dir,
            "reviewer:test",
            &first_policy.id,
            || {
                resurrect_clock.set(resurrect_clock.get() + 1);
                "never sampled".to_string()
            },
            || {
                resurrect_key_loads.set(resurrect_key_loads.get() + 1);
                throwaway_key()
            },
        )
        .unwrap_err();
        assert_eq!(resurrection.kind, ErrorKind::Custody);
        assert!(resurrection.message.contains("cannot resurrect"));
        assert_eq!(resurrect_clock.get(), 0);
        assert_eq!(resurrect_key_loads.get(), 0);

        let (second_policy, _) = draft_policy(&dir, "notes-threshold", false).unwrap();
        assert_ne!(second_policy.id, first_policy.id);
        let reopen_loads = Cell::new(0);
        let (_, _, reopened) = sign_active_policy_transactional(
            &dir,
            "reviewer:test",
            &second_policy.id,
            || REOPEN_AT.to_string(),
            || {
                reopen_loads.set(reopen_loads.get() + 1);
                throwaway_key()
            },
        )
        .unwrap();
        assert_eq!(reopen_loads.get(), 1);
        assert_eq!(reopened.action, PolicyHeadAction::Rotate);
        assert_eq!(
            reopened.policy_id.as_deref(),
            Some(second_policy.id.as_str())
        );
        assert_eq!(reopened.epoch, 3);
        let final_project = repo::load_from_path(&dir).unwrap();
        let chain = current_policy_head(&final_project).unwrap().unwrap();
        assert_eq!(chain, reopened);
        assert_eq!(
            load_active_policy(&dir).unwrap().unwrap().policy.id,
            second_policy.id
        );
    }

    #[test]
    fn transactional_sign_replace_and_sign_preserves_the_completed_history_chain() {
        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        register_transaction_reviewer(&dir);

        let (first, _) = draft_policy(&dir, "witness-rederivation", false).unwrap();
        let activated = transactionally_sign_policy(&dir, &first.id, "2099-03-01T00:00:00Z");
        assert_eq!(activated.action, PolicyHeadAction::Activate);

        let (second, replaced) = draft_policy(&dir, "notes-threshold", true).unwrap();
        assert!(replaced);
        assert!(!active_sig_path(&dir).exists());

        // Acquiring the signing transaction's recovery barrier is the
        // regression: the draft must supply the exact active.sig File->Absent
        // edge rather than leave the completed signing journal looking corrupt.
        let rotated = transactionally_sign_policy(&dir, &second.id, "2099-03-01T00:00:01Z");
        assert_eq!(rotated.action, PolicyHeadAction::Rotate);
        assert_eq!(rotated.epoch, activated.epoch + 1);
        assert_eq!(rotated.policy_id.as_deref(), Some(second.id.as_str()));
        assert_eq!(
            load_active_policy(&dir).unwrap().unwrap().policy.id,
            second.id
        );
    }

    #[test]
    fn committed_policy_draft_recovers_as_an_exact_retry_without_epoch_churn() {
        use crate::frontier_txn::FrontierTxnStep;

        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        register_transaction_reviewer(&dir);
        let (first, _) = draft_policy(&dir, "witness-rederivation", false).unwrap();
        transactionally_sign_policy(&dir, &first.id, "2099-03-01T01:00:00Z");

        set_policy_draft_install_failpoint(Some(FrontierTxnStep::AfterInstallWrite { index: 0 }));
        let error = draft_policy(&dir, "notes-threshold", true).unwrap_err();
        assert!(
            error
                .message
                .contains("injected frontier transaction failure"),
            "{}",
            error.message
        );

        // The same command first completes the marker-bound delta, then
        // recognizes the completed result instead of drafting epoch + 1 again.
        let (recovered, replaced) = draft_policy(&dir, "notes-threshold", true).unwrap();
        assert!(replaced);
        assert_eq!(recovered.epoch, first.epoch + 1);
        let active_bytes = std::fs::read(active_path(&dir)).unwrap();
        let (retry, retry_replaced) = draft_policy(&dir, "notes-threshold", true).unwrap();
        assert!(retry_replaced);
        assert_eq!(retry, recovered);
        assert_eq!(std::fs::read(active_path(&dir)).unwrap(), active_bytes);
        assert!(!active_sig_path(&dir).exists());
    }

    #[test]
    fn policy_ceremony_retries_pre_marker_and_post_marker_without_rereading_key() {
        use std::cell::Cell;

        for failpoint in [1, 2] {
            let tmp = TempDir::new().unwrap();
            let dir = init_frontier(&tmp);
            register_transaction_reviewer(&dir);
            let (policy, _) = draft_policy(&dir, "witness-rederivation", false).unwrap();
            let before =
                vela_protocol::canonical::to_canonical_bytes(&repo::load_from_path(&dir).unwrap())
                    .unwrap();
            let first_key_loads = Cell::new(0);
            set_policy_ceremony_failpoint(failpoint);
            let error = sign_active_policy_transactional(
                &dir,
                "reviewer:test",
                &policy.id,
                || "2099-02-01T00:00:00Z".to_string(),
                || {
                    first_key_loads.set(first_key_loads.get() + 1);
                    throwaway_key()
                },
            )
            .unwrap_err();
            set_policy_ceremony_failpoint(0);
            assert!(error.message.contains("injected policy ceremony failure"));
            assert_eq!(first_key_loads.get(), 1);
            assert_eq!(
                vela_protocol::canonical::to_canonical_bytes(&repo::load_from_path(&dir).unwrap())
                    .unwrap(),
                before,
                "failpoint {failpoint} changed canonical state before install"
            );
            assert!(!active_sig_path(&dir).exists());

            let retry_clock = Cell::new(0);
            let retry_key_loads = Cell::new(0);
            let (recovered_policy, _, head) = sign_active_policy_transactional(
                &dir,
                "reviewer:test",
                &policy.id,
                || {
                    retry_clock.set(retry_clock.get() + 1);
                    "never sampled".to_string()
                },
                || {
                    retry_key_loads.set(retry_key_loads.get() + 1);
                    throwaway_key()
                },
            )
            .unwrap();
            assert_eq!(recovered_policy.id, policy.id);
            assert_eq!(head.action, PolicyHeadAction::Activate);
            assert_eq!(retry_clock.get(), 0);
            assert_eq!(retry_key_loads.get(), 0);
            assert_eq!(
                current_policy_head(&repo::load_from_path(&dir).unwrap())
                    .unwrap()
                    .unwrap(),
                head
            );
        }
    }

    #[test]
    fn dry_run_is_conservative_and_never_permits() {
        // Pure in-memory: the dry-run must not need (or touch) a store.
        let mut project = vela_protocol::project::assemble("t", vec![], 0, 0, "test");
        project.proposals.push(new_proposal(
            "finding.note",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_x".to_string(),
            },
            "agent:scout",
            "agent",
            "observation",
            json!({"text": "a sidon observation"}),
            Vec::new(),
            Vec::new(),
        ));
        project.proposals.push(new_proposal(
            "finding.review",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_y".to_string(),
            },
            "agent:prover",
            "agent",
            "exact witness",
            json!({"assertion": {"text": "sidon set lower bound"}}),
            Vec::new(),
            Vec::new(),
        ));

        let (rules, _) = template_policy("notes-threshold").unwrap();
        let mut policy = AcceptancePolicy {
            schema: "vela.acceptance_policy.v0.1".to_string(),
            id: String::new(),
            frontier_id: "vfr_test".to_string(),
            epoch: 1,
            issued_by: vec!["reviewer:test".to_string()],
            quorum: Quorum {
                threshold: 1,
                eligible_roles: vec!["steward".to_string()],
            },
            rules,
            default: Outcome::Defer,
            expires_at: "2099-12-31T23:59:59Z".to_string(),
            revocation_ref: None,
        };
        policy.id = policy.content_address();

        let rows = evaluate_pending(&project, &policy, AT, None);
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter().all(|r| r.outcome != Outcome::Permit),
            "the maximally-cautious context can never permit"
        );
        let note = rows.iter().find(|r| r.kind == "finding.note").unwrap();
        assert_eq!(note.claim_class, "finding_note");
        let review = rows.iter().find(|r| r.kind == "finding.review").unwrap();
        assert_eq!(review.claim_class, "sidon_lower_bound");
    }

    /// The template must permit what `vela land` actually produces — the
    /// exact PolicyContext a draft receipt with a passing verifier run is
    /// stamped with (workflow.rs): class receipt_theoretical, A2, text
    /// mutated, independence NOT satisfied. A template that never fires
    /// is a ceremony that delegates nothing.
    #[test]
    fn statement_drafts_template_permits_a_landed_receipt() {
        let (rules, _) = template_policy("statement-drafts").unwrap();
        let mut policy = AcceptancePolicy {
            schema: "vela.acceptance_policy.v0.1".to_string(),
            id: String::new(),
            frontier_id: "vfr_test".to_string(),
            epoch: 1,
            issued_by: vec!["reviewer:test".to_string()],
            quorum: Quorum {
                threshold: 1,
                eligible_roles: vec!["steward".to_string()],
            },
            rules,
            default: Outcome::Defer,
            expires_at: "2099-12-31T23:59:59Z".to_string(),
            revocation_ref: None,
        };
        policy.id = policy.content_address();

        let ctx = vela_protocol::acceptance_policy::PolicyContext {
            claim_class: "receipt_theoretical".to_string(),
            assurance_level: 2,
            impact_tier: 1,
            changed_findings: 1,
            downstream_dependents: 0,
            assertion_text_mutated: true,
            target_contested: false,
            governance_mutation: false,
            independence_satisfied: false,
            method_integrity_sound: true,
            credential_valid: true,
            has_unknown_fields: false,
            replayability: "unknown".to_string(),
        };
        let d = vela_protocol::acceptance_policy::evaluate(&policy, &ctx, AT);
        assert_eq!(
            d.outcome,
            Outcome::Permit,
            "a landed statement draft must route through this lane: {:?}",
            d.reasons
        );
    }

    /// The lean-rederivation lane: a kernel-clean Lean re-derivation
    /// (class receipt_lean_kernel_clean, A2, method-sound) permits, but the
    /// SAME class with method integrity NOT sound — a dirty/unlisted axiom
    /// the land path refused to certify — must defer. Signing this policy
    /// delegates fidelity; it never delegates a proof that used sorryAx.
    #[test]
    fn lean_rederivation_permits_kernel_clean_defers_dirty() {
        let (rules, _) = template_policy("lean-rederivation").unwrap();
        let mut policy = AcceptancePolicy {
            schema: "vela.acceptance_policy.v0.1".to_string(),
            id: String::new(),
            frontier_id: "vfr_test".to_string(),
            epoch: 1,
            issued_by: vec!["reviewer:test".to_string()],
            quorum: Quorum {
                threshold: 1,
                eligible_roles: vec!["steward".to_string()],
            },
            rules,
            default: Outcome::Defer,
            expires_at: "2099-12-31T23:59:59Z".to_string(),
            revocation_ref: None,
        };
        policy.id = policy.content_address();

        let ctx = |method_integrity_sound: bool| vela_protocol::acceptance_policy::PolicyContext {
            claim_class: "receipt_lean_kernel_clean".to_string(),
            assurance_level: 2,
            impact_tier: 1,
            changed_findings: 1,
            downstream_dependents: 0,
            assertion_text_mutated: true,
            target_contested: false,
            governance_mutation: false,
            independence_satisfied: false,
            method_integrity_sound,
            credential_valid: true,
            has_unknown_fields: false,
            replayability: "exact".to_string(),
        };
        let clean = vela_protocol::acceptance_policy::evaluate(&policy, &ctx(true), AT);
        assert_eq!(
            clean.outcome,
            Outcome::Permit,
            "a kernel-clean Lean re-derivation must land through this lane: {:?}",
            clean.reasons
        );
        let dirty = vela_protocol::acceptance_policy::evaluate(&policy, &ctx(false), AT);
        assert_eq!(
            dirty.outcome,
            Outcome::Defer,
            "no method integrity (a refused axiom set) must defer to the human"
        );
    }

    /// The search-witness lane is the harvest's autonomy edge: a frozen-
    /// verified computational witness (assurance 2) permits, but the SAME
    /// claim with no passing verifier run (assurance 0) must defer. This is
    /// the exact boundary Lane A rides.
    #[test]
    fn search_witness_permits_at_a2_defers_at_a0() {
        let (rules, _) = template_policy("search-witness").unwrap();
        let mut policy = AcceptancePolicy {
            schema: "vela.acceptance_policy.v0.1".to_string(),
            id: String::new(),
            frontier_id: "vfr_test".to_string(),
            epoch: 1,
            issued_by: vec!["reviewer:test".to_string()],
            quorum: Quorum {
                threshold: 1,
                eligible_roles: vec!["steward".to_string()],
            },
            rules,
            default: Outcome::Defer,
            expires_at: "2099-12-31T23:59:59Z".to_string(),
            revocation_ref: None,
        };
        policy.id = policy.content_address();

        // The EXACT context `vela land` stamps for a computational receipt
        // with a passing verifier run (workflow.rs): has_pass → assurance 2
        // and method_integrity_sound; assertion_text_mutated is ALWAYS true
        // for a new receipt (a new claim is new text). The lane must permit
        // under that real context, not a hand-tuned one.
        let verified = vela_protocol::acceptance_policy::PolicyContext {
            claim_class: "receipt_computational".to_string(),
            assurance_level: 2,
            impact_tier: 1,
            changed_findings: 1,
            downstream_dependents: 0,
            assertion_text_mutated: true,
            target_contested: false,
            governance_mutation: false,
            independence_satisfied: false,
            method_integrity_sound: true,
            credential_valid: true,
            has_unknown_fields: false,
            replayability: "unknown".to_string(),
        };
        assert_eq!(
            vela_protocol::acceptance_policy::evaluate(&policy, &verified, AT).outcome,
            Outcome::Permit,
            "a frozen-verified computational witness must auto-land"
        );

        // Same claim, no passing verifier run: assurance 0, method integrity
        // unproven. The witness never cleared a verifier, so it must defer.
        let unverified = vela_protocol::acceptance_policy::PolicyContext {
            assurance_level: 0,
            method_integrity_sound: false,
            ..verified
        };
        assert_eq!(
            vela_protocol::acceptance_policy::evaluate(&policy, &unverified, AT).outcome,
            Outcome::Defer,
            "an unverified computational claim must NOT auto-land"
        );
    }

    /// Rotating a signed policy must carry its rules into the new epoch:
    /// opening one lane may never close another as a side effect.
    #[test]
    fn replace_carries_the_prior_rules_forward() {
        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        register_transaction_reviewer(&dir);
        let (first, _) = draft_policy(&dir, "witness-rederivation", false).unwrap();
        transactionally_sign_policy(&dir, &first.id, "2099-04-01T00:00:00Z");

        let (second, replaced) = draft_policy(&dir, "statement-drafts", true).unwrap();
        assert!(replaced);
        assert_eq!(second.epoch, first.epoch + 1);
        let ids: Vec<&str> = second.rules.iter().map(|r| r.id.as_str()).collect();
        assert!(
            ids.contains(&"witness-rederivation-v1"),
            "the standing grant must survive the rotation: {ids:?}"
        );
        assert!(ids.contains(&"statement-drafts-v1"), "{ids:?}");
        // The outgoing signed pair is snapshotted under its content address.
        assert!(
            policies_dir(&dir)
                .join(format!("{}.json", first.id))
                .exists()
        );
        // Re-drafting the SAME template supersedes, not duplicates.
        transactionally_sign_policy(&dir, &second.id, "2099-04-01T00:00:01Z");
        let (third, _) = draft_policy(&dir, "statement-drafts", true).unwrap();
        let dup = third
            .rules
            .iter()
            .filter(|r| r.id == "statement-drafts-v1")
            .count();
        assert_eq!(dup, 1, "same-id template rule supersedes the carried one");
    }

    #[test]
    fn log_finds_stamped_events() {
        let mut project = vela_protocol::project::assemble("t", vec![], 0, 0, "test");
        let mut lane_payload = serde_json::Map::new();
        lane_payload.insert(
            POLICY_LANE_PAYLOAD_KEY.to_string(),
            json!({
                "policy_id": "vap_abc",
                "rule_ids": ["witness-rederivation-v1"],
                "certificate": {"proposal_id": "vpr_9"},
                "context": {},
            }),
        );
        let lane_event: vela_protocol::events::StateEvent = serde_json::from_value(json!({
            "id": "vev_lane1",
            "kind": "finding.reviewed",
            "target": {"type": "finding", "id": "vf_x"},
            "actor": {"id": "policy:vap_abc", "type": "agent"},
            "timestamp": AT,
            "reason": "policy permit",
            "before_hash": "h0",
            "after_hash": "h1",
            "payload": serde_json::Value::Object(lane_payload),
        }))
        .unwrap();
        let plain_event: vela_protocol::events::StateEvent = serde_json::from_value(json!({
            "id": "vev_plain",
            "kind": "finding.reviewed",
            "target": {"type": "finding", "id": "vf_x"},
            "actor": {"id": "reviewer:will", "type": "human"},
            "timestamp": AT,
            "reason": "key-signed accept",
            "before_hash": "h1",
            "after_hash": "h2",
            "payload": {},
        }))
        .unwrap();
        project.events.push(lane_event);
        project.events.push(plain_event);

        let rows = lane_admissions(&project);
        assert_eq!(rows.len(), 1, "only the stamped event is an admission");
        assert_eq!(rows[0].policy_id, "vap_abc");
        assert_eq!(rows[0].event_id, "vev_lane1");
        assert_eq!(rows[0].proposal_id, "vpr_9");
        assert_eq!(rows[0].rule_ids, vec!["witness-rederivation-v1"]);
        assert_eq!(rows[0].timestamp, AT);
    }
}
