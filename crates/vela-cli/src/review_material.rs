//! Pure derivation of decision-critical review facts.
//!
//! Producer-reported Receipt v1 verifier runs are provenance. They never enter
//! this builder. Assurance, independence, and method integrity derive only from
//! durable verifier attachments through the protocol gate; missing inputs make
//! the resulting [`PolicyContext`] more conservative.

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vela_protocol::acceptance_policy::PolicyContext;
use vela_protocol::project::Project;
use vela_protocol::proposals::StateProposal;
pub(crate) use vela_protocol::proposals::policy_accept::proposal_claim_class;
#[cfg(test)]
use vela_protocol::proposals::policy_accept::{
    PolicyContextInputs, derive_policy_context, receipt_producer_credential_valid,
};
use vela_protocol::receipt_v1::ReceiptV1;

/// Derive the policy facts for a proposal already present in a frontier.
///
/// The optional receipt is trusted only when its canonical root and claim body
/// match the proposal's typed `vela_submission` links. Missing, unreadable, or
/// mismatched receipt material cannot raise assurance, credential validity, or
/// body-binding status. This makes queue, policy-preview, and policy-suggestion
/// projections agree with the landing derivation without letting a projection
/// manufacture facts that were never retained.
pub(crate) fn derive_existing_proposal_policy_context(
    project: &Project,
    proposal_id: &str,
    receipt: Option<&ReceiptV1>,
    decision_time: &str,
) -> PolicyContext {
    vela_protocol::proposals::policy_accept::derive_existing_proposal_policy_context(
        project,
        proposal_id,
        receipt,
        decision_time,
    )
}

/// Load the exact Receipt v1 named by a proposal's typed submission links.
/// Any path, symlink, parse, or root mismatch returns `None`; callers then use
/// the conservative branch of [`derive_existing_proposal_policy_context`].
pub(crate) fn frontier_receipt_for_proposal(
    frontier: &Path,
    proposal: &StateProposal,
) -> Option<ReceiptV1> {
    let submission = proposal.payload.get("vela_submission")?;
    let receipt_path = submission.get("receipt_path")?.as_str()?;
    let declared_root = submission.get("receipt_root")?.as_str()?;
    let relative = Path::new(receipt_path);
    if !receipt_path.starts_with("records/receipts/sha256/")
        || relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return None;
    }
    let bytes = crate::bounded_file::read_bounded_frontier_file(
        frontier,
        relative,
        crate::bounded_file::RECEIPT_MAX_BYTES,
        "retained receipt",
    )
    .ok()?;
    let receipt = ReceiptV1::parse(&bytes).ok()?;
    (receipt.canonical_root().ok()?.as_str() == declared_root).then_some(receipt)
}

pub(crate) const REVIEW_PAGE_DEFAULT: usize = 25;
pub(crate) const REVIEW_PAGE_MAX: usize = 100;
const REVIEW_CURSOR_DOMAIN: &[u8] = b"vela.review-cursor.internal.v1";
const REVIEW_CURSOR_MAX_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Default)]
pub(crate) struct ReviewRequest {
    pub(crate) limit: Option<usize>,
    pub(crate) cursor: Option<String>,
    pub(crate) proposal_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReviewPage {
    /// Catalog/state snapshot used for keyset continuation. Retained receipt
    /// availability is deliberately re-observed only for selected items and
    /// is bound by each item's decision_facts_root; this pagination root is
    /// navigation state, never decision authority.
    pub(crate) snapshot_root: String,
    pub(crate) event_log_root: String,
    pub(crate) observed_at: String,
    pub(crate) total: usize,
    pub(crate) returned: usize,
    pub(crate) items: Vec<vela_edge::decision_brief::ReviewSnapshot>,
    pub(crate) next_cursor: Option<String>,
    pub(crate) receipts_opened: usize,
}

/// Lock-neutral review material for an exact caller-selected proposal set.
///
/// The caller must already own the frontier recovery barrier and must pass the
/// `Project` loaded while that barrier is held. This seam deliberately does
/// not acquire a second (non-reentrant) barrier. It reuses the same bounded
/// receipt, policy, Engine, and Decision Brief derivation as the paginated read
/// projection, while preserving the caller's proposal order for a Decision
/// Plan signing preimage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LockedReviewSelection {
    pub(crate) event_log_root: String,
    pub(crate) active_policy_snapshot_root: String,
    pub(crate) engine_policy_observation_root: String,
    pub(crate) items: Vec<vela_edge::decision_brief::ReviewSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReviewProjectionError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

impl ReviewProjectionError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ReviewProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

#[derive(Debug, Clone, Serialize)]
struct PendingReviewLeaf {
    created_at: String,
    proposal_id: String,
    proposal_root: String,
    receipt_path: Option<String>,
    declared_receipt_root: Option<String>,
}

impl PendingReviewLeaf {
    fn key(&self) -> (&str, &str) {
        (&self.created_at, &self.proposal_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Opaque pagination state, not an authorization token.
///
/// Its checksum rejects accidental corruption and its snapshot/anchor checks
/// prevent silent gaps across ordinary continuation. A caller can always
/// choose where to navigate; Slice 4 therefore rederives every fact under the
/// frontier lock before any key access and never trusts this cursor or a
/// Decision Brief as authority.
struct ReviewCursor {
    version: u8,
    snapshot_root: String,
    filter_root: String,
    order: String,
    observed_at: String,
    after_created_at: String,
    after_proposal_id: String,
    after_proposal_root: String,
    page_size: usize,
}

enum LoadedReceipt {
    Parsed(ReceiptV1),
    Missing(String),
    Invalid(String),
}

impl LoadedReceipt {
    fn material(&self) -> vela_edge::decision_brief::ReceiptMaterial<'_> {
        match self {
            Self::Parsed(receipt) => {
                vela_edge::decision_brief::ReceiptMaterial::from_receipt(receipt)
            }
            Self::Missing(reason) => vela_edge::decision_brief::ReceiptMaterial::missing(reason),
            Self::Invalid(reason) => vela_edge::decision_brief::ReceiptMaterial::invalid(reason),
        }
    }
}

/// The single filesystem/clock/pagination seam for Decision Brief reads.
///
/// The frontier recovery barrier is held while Project, policy, the selected
/// receipts, and Engine previews are read. Ordering keys are built before any
/// receipt is opened, so a 25-item page never parses 10,000 receipts.
pub(crate) struct ReviewProjection;

impl ReviewProjection {
    pub(crate) fn page(
        frontier: &Path,
        request: ReviewRequest,
    ) -> Result<ReviewPage, ReviewProjectionError> {
        let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)
            .map_err(|error| ReviewProjectionError::new("frontier_unavailable", error))?;
        let _barrier =
            crate::frontier_txn::FrontierTxn::acquire_recovery_barrier(frontier, &journal_dir)
                .map_err(review_barrier_error)?;
        let supplied_cursor = request
            .cursor
            .as_deref()
            .map(decode_review_cursor)
            .transpose()?;
        let project = vela_protocol::repo::load_from_path(frontier)
            .map_err(|error| ReviewProjectionError::new("frontier_invalid", error))?;
        let project_root = format!(
            "sha256:{}",
            vela_protocol::canonical::sha256_canonical(&project)
                .map_err(|error| ReviewProjectionError::new("project_root_failed", error))?
        );
        let observed_at = supplied_cursor.as_ref().map_or_else(
            || chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            |cursor| cursor.observed_at.clone(),
        );
        let replay_ok = vela_protocol::reducer::verify_replay(&project).ok;
        let policy_snapshot =
            vela_protocol::acceptance_policy::load_active_policy_snapshot(frontier);
        let engine_policy_observation =
            vela_protocol::frontier_policy::engine_policy_summary_observation(frontier);

        let mut seen = std::collections::BTreeSet::new();
        let mut leaves = Vec::new();
        for proposal in project.proposals.iter().filter(|proposal| {
            proposal.status == "pending_review" && proposal.applied_event_id.is_none()
        }) {
            if !seen.insert(proposal.id.as_str()) {
                return Err(ReviewProjectionError::new(
                    "duplicate_proposal_id",
                    format!("proposal {} appears more than once", proposal.id),
                ));
            }
            let expected = vela_protocol::proposals::proposal_id(proposal);
            if proposal.id != expected {
                return Err(ReviewProjectionError::new(
                    "proposal_id_mismatch",
                    format!("stored proposal {} rederives as {expected}", proposal.id),
                ));
            }
            if request
                .proposal_id
                .as_ref()
                .is_some_and(|requested| requested != &proposal.id)
            {
                continue;
            }
            let created_at = chrono::DateTime::parse_from_rfc3339(&proposal.created_at)
                .map_err(|error| {
                    ReviewProjectionError::new(
                        "proposal_time_invalid",
                        format!("proposal {} created_at: {error}", proposal.id),
                    )
                })?
                .to_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
            let submission = proposal.payload.get("vela_submission");
            leaves.push(PendingReviewLeaf {
                created_at,
                proposal_id: proposal.id.clone(),
                proposal_root: format!(
                    "sha256:{}",
                    vela_protocol::canonical::sha256_canonical(proposal).map_err(|error| {
                        ReviewProjectionError::new("proposal_root_failed", error)
                    })?
                ),
                receipt_path: submission
                    .and_then(|value| value.get("receipt_path"))
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string),
                declared_receipt_root: submission
                    .and_then(|value| value.get("receipt_root"))
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string),
            });
        }
        leaves.sort_by(|left, right| left.key().cmp(&right.key()));

        let event_log_root = format!(
            "sha256:{}",
            vela_protocol::events::event_log_hash(&project.events)
        );
        let policy_snapshot_root = policy_snapshot_marker(&policy_snapshot);
        let snapshot_root = format!(
            "sha256:{}",
            vela_protocol::canonical::sha256_canonical(&serde_json::json!({
                "schema": "vela.review-snapshot.internal.v1",
                "project_root": project_root,
                "event_log_root": event_log_root,
                "policy_snapshot_root": policy_snapshot_root,
                "engine_policy_summary_observation": engine_policy_observation,
                "observed_at": observed_at,
                "pending": leaves,
            }))
            .map_err(|error| ReviewProjectionError::new("snapshot_root_failed", error))?
        );
        let filter_root = format!(
            "sha256:{}",
            vela_protocol::canonical::sha256_canonical(&serde_json::json!({
                "proposal_id": request.proposal_id,
                "order": "created_at_utc_then_proposal_id",
            }))
            .map_err(|error| ReviewProjectionError::new("filter_root_failed", error))?
        );
        let limit = match request.limit {
            None => REVIEW_PAGE_DEFAULT,
            Some(limit @ 1..=REVIEW_PAGE_MAX) => limit,
            Some(limit) => {
                return Err(ReviewProjectionError::new(
                    "limit_invalid",
                    format!("review page limit {limit} is outside 1..={REVIEW_PAGE_MAX}"),
                ));
            }
        };
        let after = match supplied_cursor {
            None => None,
            Some(cursor) => {
                if cursor.snapshot_root != snapshot_root {
                    return Err(ReviewProjectionError::new(
                        "stale_cursor",
                        "the frontier, pending set, or policy snapshot changed; start a fresh page",
                    ));
                }
                if cursor.filter_root != filter_root
                    || cursor.order != "created_at_utc_then_proposal_id"
                    || cursor.page_size != limit
                {
                    return Err(ReviewProjectionError::new(
                        "cursor_query_mismatch",
                        "cursor filter, order, or page size differs from this request",
                    ));
                }
                Some((
                    cursor.after_created_at,
                    cursor.after_proposal_id,
                    cursor.after_proposal_root,
                ))
            }
        };
        let (start, selected) = select_review_leaves(&leaves, after.as_ref(), limit)?;
        let has_more = start + selected.len() < leaves.len();
        let mut items = Vec::with_capacity(selected.len());
        let mut receipts_opened = 0usize;
        for leaf in &selected {
            let proposal = project
                .proposals
                .iter()
                .find(|proposal| proposal.id == leaf.proposal_id)
                .expect("catalogued proposal remains in the immutable snapshot");
            let loaded = load_receipt_material(frontier, proposal, &mut receipts_opened);
            items.push(build_review_item(
                frontier,
                &project,
                proposal,
                &loaded,
                &policy_snapshot,
                &observed_at,
                replay_ok,
            )?);
        }
        if vela_protocol::frontier_policy::engine_policy_summary_observation(frontier)
            != engine_policy_observation
        {
            return Err(ReviewProjectionError::new(
                "policy_changed_during_read",
                "Engine policy inputs changed while the review page was being derived; start a fresh page",
            ));
        }
        let next_cursor = if has_more {
            selected
                .last()
                .map(|leaf| {
                    encode_review_cursor(&ReviewCursor {
                        version: 1,
                        snapshot_root: snapshot_root.clone(),
                        filter_root: filter_root.clone(),
                        order: "created_at_utc_then_proposal_id".to_string(),
                        observed_at: observed_at.clone(),
                        after_created_at: leaf.created_at.clone(),
                        after_proposal_id: leaf.proposal_id.clone(),
                        after_proposal_root: leaf.proposal_root.clone(),
                        page_size: limit,
                    })
                })
                .transpose()?
        } else {
            None
        };
        Ok(ReviewPage {
            snapshot_root,
            event_log_root,
            observed_at,
            total: leaves.len(),
            returned: items.len(),
            items,
            next_cursor,
            receipts_opened,
        })
    }

    pub(crate) fn one(
        frontier: &Path,
        proposal_id: &str,
    ) -> Result<vela_edge::decision_brief::ReviewSnapshot, ReviewProjectionError> {
        let mut page = Self::page(
            frontier,
            ReviewRequest {
                limit: Some(1),
                cursor: None,
                proposal_id: Some(proposal_id.to_string()),
            },
        )?;
        page.items.pop().ok_or_else(|| {
            ReviewProjectionError::new(
                "proposal_not_found",
                format!("pending proposal {proposal_id} was not found"),
            )
        })
    }

    /// Render the first phase of a single-item scripted decision without
    /// acquiring the recovery barrier. This path must remain strictly
    /// read-only even when an earlier transaction is still Prepared. The
    /// Decision Plan preview builder double-reads and compares the complete
    /// input set, and confirmed execution later rederives under the barrier.
    pub(crate) fn one_read_only(
        frontier: &Path,
        proposal_id: &str,
    ) -> Result<vela_edge::decision_brief::ReviewSnapshot, ReviewProjectionError> {
        let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        Self::one_at(frontier, proposal_id, &observed_at)
    }

    /// Rebuild one scripted confirmation preview at an explicitly echoed
    /// observation instant. This keeps event timestamps and time-dependent
    /// policy inputs inside the Decision Plan root without writing a private
    /// preview ticket between processes.
    pub(crate) fn one_at(
        frontier: &Path,
        proposal_id: &str,
        observed_at: &str,
    ) -> Result<vela_edge::decision_brief::ReviewSnapshot, ReviewProjectionError> {
        let project = vela_protocol::repo::load_from_path(frontier)
            .map_err(|error| ReviewProjectionError::new("frontier_invalid", error))?;
        let mut selection = Self::selected_from_locked_project_at(
            frontier,
            &project,
            &[proposal_id.to_string()],
            observed_at,
        )?;
        selection.items.pop().ok_or_else(|| {
            ReviewProjectionError::new(
                "proposal_not_found",
                format!("pending proposal {proposal_id} was not found"),
            )
        })
    }

    /// Rederive one or many exact proposal briefs from a caller-owned locked
    /// Project snapshot, at a caller-bound observation/decision time.
    ///
    /// `proposal_ids` is ordered and duplicate-free. Every id must still name
    /// a pending, unapplied proposal in `project`; missing or newly decided
    /// proposals are typed stale input rather than silently skipped.
    pub(crate) fn selected_from_locked_project_at(
        frontier: &Path,
        project: &Project,
        proposal_ids: &[String],
        observed_at: &str,
    ) -> Result<LockedReviewSelection, ReviewProjectionError> {
        chrono::DateTime::parse_from_rfc3339(observed_at).map_err(|error| {
            ReviewProjectionError::new(
                "decision_time_invalid",
                format!("review observation time is invalid: {error}"),
            )
        })?;
        if proposal_ids.is_empty() {
            return Err(ReviewProjectionError::new(
                "proposal_set_empty",
                "a locked review selection requires at least one proposal",
            ));
        }
        if proposal_ids.len() > REVIEW_PAGE_MAX {
            return Err(ReviewProjectionError::new(
                "proposal_set_too_large",
                format!(
                    "locked review selection has {} proposals; maximum is {REVIEW_PAGE_MAX}",
                    proposal_ids.len()
                ),
            ));
        }

        let mut seen = std::collections::BTreeSet::new();
        for proposal_id in proposal_ids {
            if !seen.insert(proposal_id.as_str()) {
                return Err(ReviewProjectionError::new(
                    "duplicate_proposal_id",
                    format!("proposal {proposal_id} appears more than once"),
                ));
            }
        }

        let replay_ok = vela_protocol::reducer::verify_replay(project).ok;
        let policy_snapshot =
            vela_protocol::acceptance_policy::load_active_policy_snapshot(frontier);
        let engine_policy_observation =
            vela_protocol::frontier_policy::engine_policy_summary_observation(frontier);
        let active_policy_snapshot_root = policy_snapshot_marker_root(&policy_snapshot)?;
        let event_log_root = format!(
            "sha256:{}",
            vela_protocol::events::event_log_hash(&project.events)
        );

        let mut receipts_opened = 0usize;
        let mut items = Vec::with_capacity(proposal_ids.len());
        for proposal_id in proposal_ids {
            let proposal = project
                .proposals
                .iter()
                .find(|proposal| proposal.id == *proposal_id)
                .ok_or_else(|| {
                    ReviewProjectionError::new(
                        "proposal_not_found",
                        format!("pending proposal {proposal_id} was not found"),
                    )
                })?;
            if proposal.status != "pending_review" || proposal.applied_event_id.is_some() {
                return Err(ReviewProjectionError::new(
                    "proposal_no_longer_pending",
                    format!("proposal {proposal_id} is no longer pending and unapplied"),
                ));
            }
            let expected = vela_protocol::proposals::proposal_id(proposal);
            if proposal.id != expected {
                return Err(ReviewProjectionError::new(
                    "proposal_id_mismatch",
                    format!("stored proposal {} rederives as {expected}", proposal.id),
                ));
            }
            let loaded = load_receipt_material(frontier, proposal, &mut receipts_opened);
            items.push(build_review_item(
                frontier,
                project,
                proposal,
                &loaded,
                &policy_snapshot,
                observed_at,
                replay_ok,
            )?);
        }

        if vela_protocol::frontier_policy::engine_policy_summary_observation(frontier)
            != engine_policy_observation
        {
            return Err(ReviewProjectionError::new(
                "policy_changed_during_read",
                "Engine policy inputs changed while the locked review set was being derived",
            ));
        }

        Ok(LockedReviewSelection {
            event_log_root,
            active_policy_snapshot_root,
            engine_policy_observation_root: engine_policy_observation.root,
            items,
        })
    }
}

fn select_review_leaves<'a>(
    leaves: &'a [PendingReviewLeaf],
    after: Option<&(String, String, String)>,
    limit: usize,
) -> Result<(usize, Vec<&'a PendingReviewLeaf>), ReviewProjectionError> {
    let start = match after {
        None => 0,
        Some((created_at, proposal_id, proposal_root)) => leaves
            .iter()
            .position(|leaf| {
                leaf.key() == (created_at.as_str(), proposal_id.as_str())
                    && leaf.proposal_root == *proposal_root
            })
            .map(|index| index + 1)
            .ok_or_else(|| {
                ReviewProjectionError::new(
                    "cursor_invalid",
                    "cursor does not name an exact item in the bound review snapshot",
                )
            })?,
    };
    Ok((start, leaves.iter().skip(start).take(limit).collect()))
}

fn review_barrier_error(error: crate::frontier_txn::FrontierTxnError) -> ReviewProjectionError {
    let code = match &error {
        crate::frontier_txn::FrontierTxnError::Busy => "frontier_busy",
        crate::frontier_txn::FrontierTxnError::RecoveryRequired { .. } => {
            "frontier_recovery_required"
        }
        _ => "frontier_unavailable",
    };
    ReviewProjectionError::new(code, error.to_string())
}

fn build_review_item(
    frontier: &Path,
    project: &Project,
    proposal: &StateProposal,
    loaded: &LoadedReceipt,
    policy_snapshot: &Result<vela_protocol::acceptance_policy::ActivePolicySnapshot, String>,
    observed_at: &str,
    replay_ok: bool,
) -> Result<vela_edge::decision_brief::ReviewSnapshot, ReviewProjectionError> {
    let material = loaded.material();
    let publication = None;
    let build = |route: vela_edge::decision_brief::ReviewRoute<'_>| {
        vela_edge::decision_brief::build_review_snapshot(
            project,
            vela_edge::decision_brief::DecisionBriefInput {
                proposal_id: &proposal.id,
                receipt: material,
                route,
                observed_at,
                replay_ok,
                publication,
            },
        )
        .map_err(|error| ReviewProjectionError::new("decision_brief_invariant", error))
    };

    let Ok(snapshot) = policy_snapshot else {
        return build(vela_edge::decision_brief::ReviewRoute::unavailable(
            "broken",
            policy_snapshot.as_ref().unwrap_err(),
        ));
    };

    let policy_eligible = proposal.kind == "finding.add"
        && proposal.target.r#type == "finding"
        && matches!(loaded, LoadedReceipt::Parsed(_));
    if !policy_eligible {
        let has_submission = proposal.payload.get("vela_submission").is_some();
        let material_unavailable = matches!(
            loaded,
            LoadedReceipt::Missing(_) | LoadedReceipt::Invalid(_)
        );
        if has_submission && material_unavailable {
            return build(vela_edge::decision_brief::ReviewRoute::unavailable(
                "material_unavailable",
                "a coherent receipt-backed policy route cannot be derived",
            ));
        }
        let state = if has_submission {
            "proposal_kind_outside_policy_lane"
        } else {
            "human_review_only"
        };
        return build(vela_edge::decision_brief::ReviewRoute::human_only(
            state,
            "this proposal kind requires an explicit human decision",
        ));
    }

    let LoadedReceipt::Parsed(receipt) = loaded else {
        unreachable!("policy eligibility requires parsed receipt")
    };
    match vela_protocol::proposals::policy_accept::stage_policy_route_in_frontier_at(
        frontier,
        project,
        &proposal.id,
        receipt,
        observed_at,
        snapshot,
    ) {
        Ok(staged) => build(vela_edge::decision_brief::ReviewRoute::from_staged(&staged)),
        Err(error) => {
            let error = error.to_string();
            build(vela_edge::decision_brief::ReviewRoute::unavailable(
                match snapshot.mode {
                    vela_protocol::acceptance_policy::ActivePolicyMode::Active => "active",
                    vela_protocol::acceptance_policy::ActivePolicyMode::StagedUnsigned => {
                        "staged_unsigned"
                    }
                    vela_protocol::acceptance_policy::ActivePolicyMode::Absent => "closed",
                },
                &error,
            ))
        }
    }
}

fn load_receipt_material(
    frontier: &Path,
    proposal: &StateProposal,
    receipts_opened: &mut usize,
) -> LoadedReceipt {
    let Some(submission) = proposal.payload.get("vela_submission") else {
        return LoadedReceipt::Missing("receipt_not_applicable".to_string());
    };
    let Some(receipt_path) = submission
        .get("receipt_path")
        .and_then(serde_json::Value::as_str)
    else {
        return LoadedReceipt::Missing("receipt_path_absent".to_string());
    };
    let Some(declared_root) = submission
        .get("receipt_root")
        .and_then(serde_json::Value::as_str)
    else {
        return LoadedReceipt::Invalid("receipt_root_absent".to_string());
    };
    let relative = Path::new(receipt_path);
    if !receipt_path.starts_with("records/receipts/sha256/")
        || relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return LoadedReceipt::Invalid("receipt_path_unsafe".to_string());
    }
    let bytes = match crate::bounded_file::read_bounded_frontier_file(
        frontier,
        relative,
        crate::bounded_file::RECEIPT_MAX_BYTES,
        "retained receipt",
    ) {
        Ok(bytes) => {
            *receipts_opened = receipts_opened.saturating_add(1);
            bytes
        }
        Err(error) => {
            if error.opened {
                *receipts_opened = receipts_opened.saturating_add(1);
            }
            if error.code == "missing" {
                return LoadedReceipt::Missing("receipt_file_missing".to_string());
            }
            return LoadedReceipt::Invalid(format!("receipt_file_{}", error.code));
        }
    };
    let receipt = match ReceiptV1::parse(&bytes) {
        Ok(receipt) => receipt,
        Err(_) => return LoadedReceipt::Invalid("receipt_parse_failed".to_string()),
    };
    let actual = match receipt.canonical_root() {
        Ok(root) => root,
        Err(_) => return LoadedReceipt::Invalid("receipt_root_failed".to_string()),
    };
    if actual != declared_root {
        return LoadedReceipt::Invalid("receipt_root_mismatch".to_string());
    }
    if let Some(finding) = proposal.payload.get("finding") {
        let expected_claim = finding
            .pointer("/assertion/text")
            .and_then(serde_json::Value::as_str);
        let expected_type = finding
            .pointer("/assertion/type")
            .and_then(serde_json::Value::as_str);
        if receipt
            .as_value()
            .get("claim")
            .and_then(serde_json::Value::as_str)
            != expected_claim
        {
            return LoadedReceipt::Invalid("receipt_claim_mismatch".to_string());
        }
        if receipt
            .as_value()
            .get("type")
            .and_then(serde_json::Value::as_str)
            != expected_type
        {
            return LoadedReceipt::Invalid("receipt_type_mismatch".to_string());
        }
    }
    LoadedReceipt::Parsed(receipt)
}

fn policy_snapshot_marker(
    snapshot: &Result<vela_protocol::acceptance_policy::ActivePolicySnapshot, String>,
) -> serde_json::Value {
    match snapshot {
        Ok(snapshot) => serde_json::json!({
            "state": match snapshot.mode {
                vela_protocol::acceptance_policy::ActivePolicyMode::Active => "live",
                vela_protocol::acceptance_policy::ActivePolicyMode::StagedUnsigned => "staged_unsigned",
                vela_protocol::acceptance_policy::ActivePolicyMode::Absent => "closed",
            },
            "policy_bytes_root": snapshot.policy_bytes.as_deref().map(bytes_root),
            "signature_bytes_root": snapshot.signature_bytes.as_deref().map(bytes_root),
        }),
        Err(error) => serde_json::json!({
            "state": "broken",
            "error_root": bytes_root(error.as_bytes()),
        }),
    }
}

fn policy_snapshot_marker_root(
    snapshot: &Result<vela_protocol::acceptance_policy::ActivePolicySnapshot, String>,
) -> Result<String, ReviewProjectionError> {
    let marker = policy_snapshot_marker(snapshot);
    let value = serde_json::json!({
        "schema": "vela.active-policy-observation.internal.v1",
        "marker": marker,
    });
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&value)
            .map_err(|error| ReviewProjectionError::new("policy_snapshot_root_failed", error))?
    ))
}

fn bytes_root(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn encode_review_cursor(cursor: &ReviewCursor) -> Result<String, ReviewProjectionError> {
    let bytes = vela_protocol::canonical::to_canonical_bytes(cursor)
        .map_err(|error| ReviewProjectionError::new("cursor_encode_failed", error))?;
    let checksum = cursor_checksum(&bytes);
    Ok(format!("v1.{}.{}", hex::encode(bytes), &checksum[..16]))
}

fn decode_review_cursor(value: &str) -> Result<ReviewCursor, ReviewProjectionError> {
    if value.len() > REVIEW_CURSOR_MAX_BYTES {
        return Err(ReviewProjectionError::new(
            "cursor_invalid",
            "review cursor exceeds the bounded input limit",
        ));
    }
    let mut parts = value.split('.');
    if parts.next() != Some("v1") {
        return Err(ReviewProjectionError::new(
            "cursor_invalid",
            "unsupported review cursor version",
        ));
    }
    let encoded = parts
        .next()
        .ok_or_else(|| ReviewProjectionError::new("cursor_invalid", "cursor body is absent"))?;
    let checksum = parts
        .next()
        .ok_or_else(|| ReviewProjectionError::new("cursor_invalid", "cursor checksum is absent"))?;
    if parts.next().is_some() {
        return Err(ReviewProjectionError::new(
            "cursor_invalid",
            "cursor has trailing fields",
        ));
    }
    let bytes = hex::decode(encoded)
        .map_err(|_| ReviewProjectionError::new("cursor_invalid", "cursor body is not hex"))?;
    let expected = cursor_checksum(&bytes);
    if checksum != &expected[..16] {
        return Err(ReviewProjectionError::new(
            "cursor_invalid",
            "cursor checksum does not match",
        ));
    }
    let cursor: ReviewCursor = serde_json::from_slice(&bytes)
        .map_err(|_| ReviewProjectionError::new("cursor_invalid", "cursor JSON is invalid"))?;
    if cursor.version != 1 {
        return Err(ReviewProjectionError::new(
            "cursor_invalid",
            "cursor payload version is unsupported",
        ));
    }
    let canonical = vela_protocol::canonical::to_canonical_bytes(&cursor)
        .map_err(|error| ReviewProjectionError::new("cursor_invalid", error))?;
    if canonical != bytes {
        return Err(ReviewProjectionError::new(
            "cursor_invalid",
            "cursor payload is not canonical",
        ));
    }
    let canonical_time = |value: &str| {
        chrono::DateTime::parse_from_rfc3339(value)
            .ok()
            .map(|time| {
                time.to_utc()
                    .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
            })
    };
    if cursor.order != "created_at_utc_then_proposal_id"
        || !(1..=REVIEW_PAGE_MAX).contains(&cursor.page_size)
        || canonical_time(&cursor.observed_at).as_deref() != Some(cursor.observed_at.as_str())
        || canonical_time(&cursor.after_created_at).as_deref()
            != Some(cursor.after_created_at.as_str())
        || !is_sha256_root(&cursor.snapshot_root)
        || !is_sha256_root(&cursor.filter_root)
        || !is_sha256_root(&cursor.after_proposal_root)
        || !cursor.after_proposal_id.starts_with("vpr_")
    {
        return Err(ReviewProjectionError::new(
            "cursor_invalid",
            "cursor payload fields are invalid or non-canonical",
        ));
    }
    Ok(cursor)
}

fn is_sha256_root(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn cursor_checksum(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(REVIEW_CURSOR_DOMAIN);
    digest.update([0]);
    digest.update(bytes);
    hex::encode(digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use serde_json::json;
    use vela_protocol::bundle::{
        Assertion, Conditions, Confidence, ConfidenceKind, ConfidenceMethod, Evidence, Extraction,
        FindingBundle, Flags, Provenance,
    };
    use vela_protocol::events::{StateActor, StateTarget};
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::receipt_v1::{ArtifactInput, ReceiptBuilder, ReceiptInput};
    use vela_protocol::sign::ActorRecord;

    fn finding() -> FindingBundle {
        finding_with_text("A bounded result")
    }

    fn finding_with_text(text: &str) -> FindingBundle {
        FindingBundle::new(
            Assertion {
                text: text.to_string(),
                assertion_type: "computational".to_string(),
                entities: vec![],
                relation: None,
                direction: None,
                causal_claim: None,
                causal_evidence_grade: None,
            },
            Evidence {
                evidence_type: "computational".to_string(),
                model_system: String::new(),
                method: "producer reported".to_string(),
                replicated: false,
                replication_count: None,
                evidence_spans: vec![],
            },
            Conditions {
                text: "pending review".to_string(),
                duration: None,
            },
            Confidence {
                kind: ConfidenceKind::FrontierEpistemic,
                score: 0.3,
                basis: "producer report".to_string(),
                method: ConfidenceMethod::ExpertJudgment,
                extraction_confidence: 1.0,
            },
            Provenance {
                source_type: "model_output".to_string(),
                doi: None,
                url: None,
                title: "receipt".to_string(),
                authors: vec![],
                year: None,
                license: None,
                publisher: None,
                funders: vec![],
                extraction: Extraction {
                    method: "receipt_import".to_string(),
                    model: None,
                    model_version: None,
                    extracted_at: "2026-07-13T00:00:00Z".to_string(),
                    extractor_version: "test".to_string(),
                },
                review: None,
                contributions: vec![],
            },
            Flags::default(),
        )
    }

    fn proposal(finding: &FindingBundle) -> StateProposal {
        StateProposal {
            schema: vela_protocol::proposals::PROPOSAL_SCHEMA.to_string(),
            id: "vpr_test".to_string(),
            kind: "finding.add".to_string(),
            target: StateTarget {
                r#type: "finding".to_string(),
                id: finding.id.clone(),
            },
            actor: StateActor {
                id: "agent:test".to_string(),
                r#type: "agent".to_string(),
            },
            created_at: "2026-07-13T00:00:00Z".to_string(),
            drafted_at: None,
            reason: "test".to_string(),
            payload: json!({"finding": finding}),
            source_refs: vec![],
            status: "pending_review".to_string(),
            reviewed_by: None,
            reviewed_at: None,
            decision_reason: None,
            applied_event_id: None,
            caveats: vec![],
            agent_run: None,
        }
    }

    fn retained_receipt(finding: &FindingBundle) -> ReceiptV1 {
        retained_receipt_at(finding, "2026-07-13T00:00:00Z")
    }

    fn retained_receipt_at(finding: &FindingBundle, at: &str) -> ReceiptV1 {
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:test".to_string(),
                actor_class: ActorClass::Agent,
                created_at: at.to_string(),
            },
            &SigningKey::from_bytes(&[0x37; 32]),
        )
        .unwrap();
        let input = ReceiptInput::new(
            finding.assertion.text.clone(),
            finding.assertion.assertion_type.clone(),
            "exact".to_string(),
            vec![
                ArtifactInput::new(
                    "witnesses/result.json".to_string(),
                    "witness".to_string(),
                    Some("a".repeat(64)),
                    Some("https://example.test/result.json".to_string()),
                )
                .unwrap(),
            ],
            vec!["bounded test fixture".to_string()],
            Vec::new(),
            "agent:test".to_string(),
            at.to_string(),
            format!("sha256:{}", "b".repeat(64)),
            ".".to_string(),
            format!("vop_{}", "c".repeat(64)),
            "urn:vela:policy:none".to_string(),
        )
        .unwrap();
        ReceiptBuilder::build(input, &identity).unwrap()
    }

    fn initialized_review_frontier() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        vela_protocol::frontier_repo::initialize(
            temp.path(),
            vela_protocol::frontier_repo::InitOptions {
                name: "review-projection-test",
                template: "",
                initialize_git: false,
            },
        )
        .unwrap();
        temp
    }

    fn at_second(index: usize) -> String {
        (chrono::DateTime::parse_from_rfc3339("2026-07-13T00:00:00Z").unwrap()
            + chrono::Duration::seconds(index.try_into().unwrap()))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    fn pending_with_missing_receipt(index: usize) -> StateProposal {
        let finding = finding_with_text(&format!("bounded pending result {index}"));
        let mut proposal = proposal(&finding);
        proposal.created_at = at_second(index);
        let receipt_root = format!("sha256:{:064x}", index + 1);
        proposal.payload["vela_submission"] = json!({
            "schema": "vela.submission-links.internal.v1",
            "receipt_root": receipt_root,
            "receipt_path": format!("records/receipts/sha256/{:064x}.json", index + 1),
            "record_id": format!("vrc_{index:016x}"),
            "operation_id": format!("vop_{:064x}", index + 1),
        });
        proposal.id = vela_protocol::proposals::proposal_id(&proposal);
        proposal
    }

    fn retain_receipt_for_proposal(frontier: &Path, index: usize) -> StateProposal {
        let finding = finding_with_text(&format!("bounded retained result {index}"));
        let receipt = retained_receipt(&finding);
        let root = receipt.canonical_root().unwrap();
        let relative = format!(
            "records/receipts/sha256/{}.json",
            root.strip_prefix("sha256:").unwrap()
        );
        let absolute = frontier.join(&relative);
        std::fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        std::fs::write(&absolute, receipt.canonical_bytes().unwrap()).unwrap();

        let mut proposal = proposal(&finding);
        proposal.created_at = at_second(index);
        proposal.payload["vela_submission"] = json!({
            "schema": "vela.submission-links.internal.v1",
            "receipt_root": root,
            "receipt_path": relative,
            "record_id": format!("vrc_{index:016x}"),
            "operation_id": format!("vop_{:064x}", index + 1),
        });
        proposal.id = vela_protocol::proposals::proposal_id(&proposal);
        proposal
    }

    fn save_pending(frontier: &Path, proposals: Vec<StateProposal>) {
        let mut project = vela_protocol::repo::load_from_path(frontier).unwrap();
        project.proposals = proposals;
        vela_protocol::repo::save_to_path(frontier, &project).unwrap();
    }

    fn snapshot_proposal_id(snapshot: &vela_edge::decision_brief::ReviewSnapshot) -> String {
        serde_json::to_value(snapshot).unwrap()["sort_key"]["proposal_id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn action_eligibility(
        brief: &vela_edge::decision_brief::DecisionBrief,
        action: &str,
    ) -> String {
        serde_json::to_value(brief).unwrap()["authority"]["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["action"] == action)
            .and_then(|entry| entry["eligibility"].as_str())
            .unwrap()
            .to_string()
    }

    #[test]
    fn producer_report_without_durable_attachments_cannot_raise_assurance() {
        let finding = finding();
        let proposal = proposal(&finding);
        let context = derive_policy_context(PolicyContextInputs {
            proposal: &proposal,
            finding: &finding,
            attachments: &[],
            replayability: Some("exact"),
            receipt_is_body_bound: true,
            credential_valid: true,
            target_contested: false,
            downstream_dependents: 0,
        });
        assert_eq!(context.assurance_level, 0);
        assert!(!context.independence_satisfied);
        assert!(!context.method_integrity_sound);
        assert!(!context.has_unknown_fields);
    }

    #[test]
    fn missing_receipt_binding_and_unknown_replayability_fail_closed() {
        let finding = finding();
        let proposal = proposal(&finding);
        let context = derive_policy_context(PolicyContextInputs {
            proposal: &proposal,
            finding: &finding,
            attachments: &[],
            replayability: Some("producer-invented"),
            receipt_is_body_bound: false,
            credential_valid: false,
            target_contested: false,
            downstream_dependents: 0,
        });
        assert!(context.has_unknown_fields);
        assert_eq!(context.replayability, "unknown");
        assert!(!context.credential_valid);
    }

    #[test]
    fn existing_proposal_context_matches_landing_derivation_for_retained_receipt() {
        let temp = tempfile::tempdir().unwrap();
        let finding = finding();
        let mut proposal = proposal(&finding);
        let receipt = retained_receipt(&finding);
        let receipt_root = receipt.canonical_root().unwrap();
        let receipt_path = format!(
            "records/receipts/sha256/{}.json",
            receipt_root.strip_prefix("sha256:").unwrap()
        );
        proposal.payload["vela_submission"] = json!({
            "schema": "vela.submission-links.internal.v1",
            "receipt_root": receipt_root,
            "receipt_path": receipt_path.clone(),
            "record_id": "vrc_0123456789abcdef",
            "operation_id": format!("vop_{}", "c".repeat(64)),
        });
        let absolute = temp.path().join(&receipt_path);
        std::fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        std::fs::write(&absolute, receipt.canonical_bytes().unwrap()).unwrap();
        let mut project = vela_protocol::project::assemble("test", vec![], 0, 0, "test");
        project.actors.push(ActorRecord {
            id: "agent:test".to_string(),
            public_key: hex::encode(
                SigningKey::from_bytes(&[0x37; 32])
                    .verifying_key()
                    .to_bytes(),
            ),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-12T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        project.proposals.push(proposal.clone());

        let loaded = frontier_receipt_for_proposal(temp.path(), &proposal)
            .expect("typed retained receipt must load");
        let decision_time = "2026-07-13T01:00:00Z";
        let actual = derive_existing_proposal_policy_context(
            &project,
            &proposal.id,
            Some(&loaded),
            decision_time,
        );
        let expected = vela_protocol::proposals::policy_accept::derive_submission_policy_context(
            &project,
            &proposal.id,
            &loaded,
            decision_time,
        )
        .expect("retained receipt should satisfy the strict landing derivation");

        assert_eq!(actual, expected);
        assert_eq!(actual.claim_class, "receipt_computational");
        assert!(actual.credential_valid);
        assert!(!actual.has_unknown_fields);
    }

    #[test]
    fn future_receipt_time_cannot_trigger_an_optimistic_projection_fallback() {
        let finding = finding();
        let receipt = retained_receipt_at(&finding, "2026-07-14T00:00:00Z");
        let mut proposal = proposal(&finding);
        proposal.payload["vela_submission"] = json!({
            "schema": "vela.submission-links.internal.v1",
            "receipt_root": receipt.canonical_root().unwrap(),
            "receipt_path": format!(
                "records/receipts/sha256/{}.json",
                receipt
                    .canonical_root()
                    .unwrap()
                    .strip_prefix("sha256:")
                    .unwrap()
            ),
            "record_id": "vrc_0123456789abcdef",
            "operation_id": format!("vop_{}", "c".repeat(64)),
        });
        let mut project = vela_protocol::project::assemble("test", vec![], 0, 0, "test");
        project.proposals.push(proposal.clone());

        let context = derive_existing_proposal_policy_context(
            &project,
            &proposal.id,
            Some(&receipt),
            "2026-07-13T01:00:00Z",
        );

        assert_eq!(context.claim_class, "receipt_computational");
        assert_eq!(
            context,
            PolicyContext {
                claim_class: "receipt_computational".to_string(),
                ..PolicyContext::default()
            }
        );
    }

    #[test]
    fn self_signed_producer_binding_needs_live_frontier_registration() {
        let finding = finding();
        let receipt = retained_receipt(&finding);
        let mut project = vela_protocol::project::assemble("test", vec![], 0, 0, "test");
        let decision_time = "2026-07-13T01:00:00Z";

        assert!(
            !receipt_producer_credential_valid(&project, &receipt, decision_time),
            "proof of possession alone is not frontier credential authority"
        );

        project.actors.push(ActorRecord {
            id: "agent:test".to_string(),
            public_key: hex::encode(
                SigningKey::from_bytes(&[0x37; 32])
                    .verifying_key()
                    .to_bytes(),
            ),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-12T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        assert!(receipt_producer_credential_valid(
            &project,
            &receipt,
            decision_time
        ));

        project.actors[0].algorithm = "not-ed25519".to_string();
        assert!(
            !receipt_producer_credential_valid(&project, &receipt, decision_time),
            "an actor record with the wrong algorithm cannot authorize an Ed25519 binding"
        );
        project.actors[0].algorithm = "ed25519".to_string();

        project.actors[0].created_at = "2026-07-13T00:30:00Z".to_string();
        assert!(
            !receipt_producer_credential_valid(&project, &receipt, decision_time),
            "registration after the producer binding cannot confer retroactive authority"
        );
        project.actors[0].created_at = "2026-07-12T00:00:00Z".to_string();

        project.actors[0].revoked_at = Some("2026-07-13T00:30:00Z".to_string());
        assert!(
            !receipt_producer_credential_valid(&project, &receipt, decision_time),
            "a key revoked before the decision must fail closed"
        );
    }

    #[test]
    fn existing_proposal_context_fails_closed_when_review_material_is_missing() {
        let finding = finding();
        let mut proposal = proposal(&finding);
        proposal.payload["finding"] = json!({
            "assertion": {
                "text": finding.assertion.text,
                "type": "theoretical",
            }
        });
        let mut project = vela_protocol::project::assemble("test", vec![], 0, 0, "test");
        project.proposals.push(proposal.clone());

        let context = derive_existing_proposal_policy_context(
            &project,
            &proposal.id,
            None,
            "2026-07-13T01:00:00Z",
        );

        assert_eq!(context.claim_class, "receipt_theoretical");
        assert_eq!(context.assurance_level, 0);
        assert_eq!(context.impact_tier, 4);
        assert_eq!(context.changed_findings, u32::MAX);
        assert_eq!(context.downstream_dependents, u32::MAX);
        assert!(context.has_unknown_fields);
        assert!(context.target_contested);
        assert!(!context.credential_valid);
        assert!(!context.independence_satisfied);
        assert!(!context.method_integrity_sound);
    }

    #[test]
    fn review_projection_pages_stably_and_rejects_changed_or_tampered_cursors() {
        let temp = initialized_review_frontier();
        let proposals = (0..3).map(pending_with_missing_receipt).collect::<Vec<_>>();
        let expected = proposals
            .iter()
            .map(|proposal| proposal.id.clone())
            .collect::<Vec<_>>();
        save_pending(temp.path(), proposals);

        let first = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(2),
                ..ReviewRequest::default()
            },
        )
        .unwrap();
        assert_eq!(first.total, 3);
        assert_eq!(first.returned, 2);
        assert_eq!(first.receipts_opened, 0);
        assert_eq!(
            first
                .items
                .iter()
                .map(snapshot_proposal_id)
                .collect::<Vec<_>>(),
            expected[..2]
        );
        let cursor = first.next_cursor.clone().expect("first page continues");

        let second = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(2),
                cursor: Some(cursor.clone()),
                proposal_id: None,
            },
        )
        .unwrap();
        assert_eq!(second.returned, 1);
        assert_eq!(snapshot_proposal_id(&second.items[0]), expected[2]);
        assert!(second.next_cursor.is_none());

        let mut tampered = cursor.clone().into_bytes();
        let last = tampered.last_mut().unwrap();
        *last = if *last == b'0' { b'1' } else { b'0' };
        let error = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(2),
                cursor: Some(String::from_utf8(tampered).unwrap()),
                proposal_id: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "cursor_invalid");

        let mut changed = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        changed.proposals.push(pending_with_missing_receipt(4));
        vela_protocol::repo::save_to_path(temp.path(), &changed).unwrap();
        let error = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(2),
                cursor: Some(cursor),
                proposal_id: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "stale_cursor");
    }

    #[test]
    fn engine_policy_inputs_are_bound_into_review_continuations() {
        let temp = initialized_review_frontier();
        save_pending(
            temp.path(),
            vec![
                pending_with_missing_receipt(0),
                pending_with_missing_receipt(1),
            ],
        );
        let first = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(1),
                ..ReviewRequest::default()
            },
        )
        .unwrap();
        let cursor = first.next_cursor.expect("two items require continuation");

        let policy_dir = temp.path().join(".vela/policy");
        std::fs::create_dir_all(&policy_dir).unwrap();
        std::fs::write(
            policy_dir.join("review_policy.md"),
            b"# Review policy\n\nHuman review remains required.\n",
        )
        .unwrap();

        let error = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(1),
                cursor: Some(cursor),
                proposal_id: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "stale_cursor");
    }

    #[test]
    fn missing_and_invalid_receipts_remain_visible_and_only_block_accept() {
        let temp = initialized_review_frontier();
        let missing = pending_with_missing_receipt(0);
        let mut invalid = pending_with_missing_receipt(1);
        let relative = invalid.payload["vela_submission"]["receipt_path"]
            .as_str()
            .unwrap()
            .to_string();
        let absolute = temp.path().join(relative);
        std::fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        std::fs::write(&absolute, b"not a receipt").unwrap();
        invalid.id = vela_protocol::proposals::proposal_id(&invalid);
        save_pending(temp.path(), vec![missing.clone(), invalid.clone()]);

        let page = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(10),
                ..ReviewRequest::default()
            },
        )
        .unwrap();
        assert_eq!(page.returned, 2);
        assert_eq!(page.receipts_opened, 1);
        assert_eq!(
            page.items
                .iter()
                .map(snapshot_proposal_id)
                .collect::<Vec<_>>(),
            vec![missing.id, invalid.id]
        );
        for item in &page.items {
            assert_eq!(action_eligibility(&item.brief, "accept"), "blocked");
            assert_eq!(action_eligibility(&item.brief, "reject"), "available");
            assert!(
                !serde_json::to_value(&item.brief).unwrap()["missing"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn review_projection_selects_and_bounds_the_page_before_opening_receipts() {
        let temp = initialized_review_frontier();
        let proposals = (0..125)
            .map(|index| retain_receipt_for_proposal(temp.path(), index))
            .collect::<Vec<_>>();
        save_pending(temp.path(), proposals);

        let first = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(REVIEW_PAGE_MAX),
                ..ReviewRequest::default()
            },
        )
        .unwrap();
        assert_eq!(first.total, 125);
        assert_eq!(first.returned, REVIEW_PAGE_MAX);
        assert_eq!(first.receipts_opened, REVIEW_PAGE_MAX);
        assert!(first.next_cursor.is_some());

        let bounded = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(25),
                ..ReviewRequest::default()
            },
        )
        .unwrap();
        assert_eq!(bounded.returned, 25);
        assert_eq!(bounded.receipts_opened, 25);
    }

    #[test]
    fn review_projection_rejects_invalid_limits_oversized_cursors_and_forged_anchors() {
        let temp = initialized_review_frontier();
        save_pending(
            temp.path(),
            (0..3).map(pending_with_missing_receipt).collect(),
        );
        for limit in [0, REVIEW_PAGE_MAX + 1] {
            let error = ReviewProjection::page(
                temp.path(),
                ReviewRequest {
                    limit: Some(limit),
                    ..ReviewRequest::default()
                },
            )
            .unwrap_err();
            assert_eq!(error.code, "limit_invalid");
        }
        let error = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(1),
                cursor: Some("x".repeat(REVIEW_CURSOR_MAX_BYTES + 1)),
                proposal_id: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "cursor_invalid");

        let first = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(1),
                ..ReviewRequest::default()
            },
        )
        .unwrap();
        let mut forged = decode_review_cursor(first.next_cursor.as_deref().unwrap()).unwrap();
        forged.after_created_at = "2026-07-13T00:00:00.500000000Z".to_string();
        forged.after_proposal_id = "vpr_forged".to_string();
        forged.after_proposal_root = format!("sha256:{}", "f".repeat(64));
        let error = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(1),
                cursor: Some(encode_review_cursor(&forged).unwrap()),
                proposal_id: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "cursor_invalid");
    }

    #[test]
    fn selected_receipt_availability_is_reobserved_and_bound_per_item() {
        let temp = initialized_review_frontier();
        let first_proposal = retain_receipt_for_proposal(temp.path(), 0);
        let second_proposal = retain_receipt_for_proposal(temp.path(), 1);
        let second_relative = second_proposal.payload["vela_submission"]["receipt_path"]
            .as_str()
            .unwrap()
            .to_string();
        let second_path = temp.path().join(&second_relative);
        let second_bytes = std::fs::read(&second_path).unwrap();
        std::fs::remove_file(&second_path).unwrap();
        save_pending(
            temp.path(),
            vec![first_proposal.clone(), second_proposal.clone()],
        );

        let missing = ReviewProjection::one(temp.path(), &second_proposal.id).unwrap();
        assert!(missing.brief.audit.receipt_root.is_none());
        let first_page = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(1),
                ..ReviewRequest::default()
            },
        )
        .unwrap();
        let cursor = first_page.next_cursor.clone().unwrap();

        std::fs::write(&second_path, second_bytes).unwrap();
        let second_page = ReviewProjection::page(
            temp.path(),
            ReviewRequest {
                limit: Some(1),
                cursor: Some(cursor),
                proposal_id: None,
            },
        )
        .unwrap();
        assert_eq!(first_page.snapshot_root, second_page.snapshot_root);
        assert_eq!(
            snapshot_proposal_id(&second_page.items[0]),
            second_proposal.id
        );
        assert!(second_page.items[0].brief.audit.receipt_root.is_some());
        assert_ne!(
            missing.brief.audit.decision_facts_root,
            second_page.items[0].brief.audit.decision_facts_root
        );
    }

    #[test]
    fn ten_thousand_catalog_ids_page_stably_before_materialization() {
        let leaves = (0..10_000)
            .map(|index| PendingReviewLeaf {
                created_at: at_second(index),
                proposal_id: format!("vpr_{index:016x}"),
                proposal_root: format!("sha256:{index:064x}"),
                receipt_path: Some(format!("records/receipts/sha256/{index:064x}.json")),
                declared_receipt_root: Some(format!("sha256:{index:064x}")),
            })
            .collect::<Vec<_>>();
        let mut after = None;
        let mut seen = std::collections::BTreeSet::new();
        let mut pages = 0usize;
        loop {
            let (_, selected) = select_review_leaves(&leaves, after.as_ref(), 100).unwrap();
            if selected.is_empty() {
                break;
            }
            assert!(selected.len() <= REVIEW_PAGE_MAX);
            for leaf in &selected {
                assert!(seen.insert(leaf.proposal_id.clone()));
            }
            let last = selected.last().unwrap();
            after = Some((
                last.created_at.clone(),
                last.proposal_id.clone(),
                last.proposal_root.clone(),
            ));
            pages += 1;
        }
        assert_eq!(pages, 100);
        assert_eq!(seen.len(), 10_000);
    }
}
