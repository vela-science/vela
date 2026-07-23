//! Protected creation of the first administrator boundary for a native
//! Profile v1 Frontier.
//!
//! `frontier.created` establishes a structural repository identity, not an
//! administrator. This command binds the first registered human
//! administrator, the exact Git/Vela anchor, and the current exact dependency
//! set in one signed `frontier.repository_bound` event. The preview is
//! key-free. The confirmed path holds the frontier recovery lock across the
//! one-shot protected signer request, then installs the exact user-local trust
//! pin only after the repository transaction completes.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vela_edge::repository_write::{
    REPOSITORY_TRUST_ANCHOR_SCHEMA_V1, RepositoryTrustAnchorV1,
    install_repository_trust_anchor_from_home, load_repository_trust_anchor_from_home,
    verify_repository_for_write,
};
use vela_protocol::events::{EVENT_KIND_FRONTIER_REPOSITORY_BOUND, StateEvent, event_log_hash};
use vela_protocol::frontier_profile::{EffectiveFrontierAuthorityV1, FRONTIER_PROFILE_SCHEMA_V1};
use vela_protocol::frontier_repo::{FrontierProfileFile, read_repository_profile};
use vela_protocol::frontier_repository::{
    FRONTIER_REPOSITORY_BOUNDARY_SCHEMA, FrontierRepositoryBoundaryMode,
    FrontierRepositoryBoundaryPayloadV1, FrontierRepositoryTrustMode, exact_dependency_root,
    new_repository_boundary_event, repository_boundary_event_content_root,
    verify_repository_boundary_signature_only,
};

use crate::config::git_publish::{
    PublicationOutcome, PublicationState, manual_uncommitted_exact_delta,
};
use crate::frontier_txn::{
    ContentDigest, DeltaDraft, FrontierBinding, FrontierTxn, FrontierTxnPlan, FrontierTxnPlanSpec,
    InputBinding, OperationId, OperationKind, PlannedWrite,
};

const BIND_PLAN_SCHEMA: &str = "vela.repository-bind-plan.v1";
const BIND_PLAN_DOMAIN: &[u8] = b"vela.repository-bind-plan.v1\0";
const BIND_RESULT_SCHEMA: &str = "vela.repository-bind-result.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepositoryBindPlan {
    pub(crate) schema: String,
    pub(crate) ok: bool,
    pub(crate) command: String,
    pub(crate) frontier: String,
    pub(crate) frontier_id: String,
    pub(crate) frontier_name: String,
    pub(crate) profile_root: String,
    pub(crate) identity_root: String,
    pub(crate) previous_identity_event_root: String,
    pub(crate) dependency_root: String,
    pub(crate) dependency_count: usize,
    pub(crate) administrator_actor: String,
    pub(crate) administrator_public_key: String,
    pub(crate) reason: String,
    pub(crate) observed_at: String,
    pub(crate) git_commit: String,
    pub(crate) git_tree: String,
    pub(crate) event_log_root_before: String,
    pub(crate) event_count_before: u64,
    pub(crate) event_log_root_after: String,
    pub(crate) event_count_after: u64,
    pub(crate) boundary_event: StateEvent,
    pub(crate) boundary_event_content_root: String,
    pub(crate) trust_anchor: RepositoryTrustAnchorV1,
    pub(crate) trust_anchor_root: String,
    pub(crate) vela_binary_path: String,
    pub(crate) vela_binary_sha256: String,
    pub(crate) helper_sha256: String,
    pub(crate) signer_provider: String,
    pub(crate) protection_grade: String,
    pub(crate) protection_mode: vela_signer::ProtectionMode,
    pub(crate) plan_root: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepositoryBindResult {
    schema: String,
    ok: bool,
    command: String,
    plan_root: String,
    operation_id: String,
    canonical_delta_root: String,
    event_id: String,
    boundary_event_content_root: String,
    event_log_root: String,
    event_count: u64,
    trust_anchor_root: String,
    trust_anchor_path: String,
    publication: PublicationOutcome,
    next_action: String,
    replay_ok: bool,
}

fn repository_bind_result_human(frontier_id: &str, result: &RepositoryBindResult) -> String {
    let publication = match &result.publication.state {
        PublicationState::Uncommitted { reason, .. } => format!("uncommitted · {reason}"),
        PublicationState::Unchanged { commit } => format!("unchanged · {commit}"),
        PublicationState::Stale {
            candidate,
            expected,
            actual,
        } => format!("stale · candidate {candidate} expected {expected}, target is {actual}"),
        PublicationState::CommittedLocal { commit } => format!("committed locally · {commit}"),
        PublicationState::Pushed { commit, remote } => {
            format!("pushed · {commit} on {remote}")
        }
        PublicationState::Unknown { reason } => format!("unknown · {reason}"),
    };
    let next = result
        .publication
        .recovery_command
        .as_deref()
        .unwrap_or(&result.next_action);
    format!(
        "bound {}\n  event: {}\n  first boundary: {}\n  trust anchor: {}\n  operation: {}\n  canonical delta: {}\n  Git publication: {}\n  next: {}\n  after Git commit: {}",
        frontier_id,
        result.event_id,
        result.boundary_event_content_root,
        result.trust_anchor_root,
        result.operation_id,
        result.canonical_delta_root,
        publication,
        next,
        result.next_action
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BindRuntime {
    administrator_actor: String,
    administrator_public_key: String,
    vela_binary_path: PathBuf,
    vela_binary_sha256: String,
    helper_sha256: String,
    signer_provider: String,
    protection_grade: String,
    protection_mode: vela_signer::ProtectionMode,
}

fn sha256_root(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn unsigned_event_content_root(event: &StateEvent) -> String {
    sha256_root(&vela_protocol::events::event_content_preimage_bytes(event))
}

fn plan_root(plan: &RepositoryBindPlan) -> Result<String, String> {
    let mut value = serde_json::to_value(plan)
        .map_err(|error| format!("encode repository bind plan: {error}"))?;
    value
        .as_object_mut()
        .ok_or_else(|| "repository bind plan is not an object".to_string())?
        .remove("plan_root");
    let canonical = vela_protocol::canonical::to_canonical_bytes(&value)?;
    let mut digest = Sha256::new();
    digest.update(BIND_PLAN_DOMAIN);
    digest.update(canonical);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

fn verify_plan(plan: &RepositoryBindPlan) -> Result<(), String> {
    if plan.schema != BIND_PLAN_SCHEMA
        || !plan.ok
        || plan.command != "frontier.bind"
        || plan.plan_root != plan_root(plan)?
    {
        return Err("repository bind plan is malformed or has a stale root".to_string());
    }
    Ok(())
}

fn same_plan(left: &RepositoryBindPlan, right: &RepositoryBindPlan) -> Result<bool, String> {
    Ok(
        serde_json::to_value(left).map_err(|error| error.to_string())?
            == serde_json::to_value(right).map_err(|error| error.to_string())?,
    )
}

fn git_text(repo: &Path, args: &[&str]) -> Result<String, String> {
    crate::git_hardened::text(repo, args)
}

fn assert_clean_repository(frontier: &Path) -> Result<(String, String), String> {
    let top = std::fs::canonicalize(git_text(frontier, &["rev-parse", "--show-toplevel"])?)
        .map_err(|error| format!("resolve Git repository root: {error}"))?;
    if top != frontier {
        return Err(format!(
            "frontier bind must name the Git repository root {}, not {}",
            top.display(),
            frontier.display()
        ));
    }
    let dirt = vela_edge::git_read::dirty_worktree_paths(frontier, true)?;
    if !dirt.is_empty() {
        return Err(format!(
            "frontier bind requires a clean checkout with no tracked or untracked changes; found {}",
            dirt.join(", ")
        ));
    }
    Ok((
        git_text(frontier, &["rev-parse", "--verify", "HEAD^{commit}"])?,
        git_text(frontier, &["rev-parse", "--verify", "HEAD^{tree}"])?,
    ))
}

fn production_runtime() -> Result<BindRuntime, String> {
    let identity = crate::cli_identity::load_identity().ok_or_else(|| {
        "no identity configured; run `vela id create --handle <name>`".to_string()
    })?;
    if identity.actor_type != "human"
        || identity.actor_id.starts_with("agent:")
        || identity.actor_id.starts_with("ci:")
    {
        return Err(
            "frontier bind requires the configured reviewer: or steward: human identity"
                .to_string(),
        );
    }
    let signer = crate::cli_identity::protected_signer_profile()?;
    let pin = crate::config::binary_pin::verify_for_ceremony()?
        .ok_or_else(|| {
            "protected frontier binding requires an exact Vela binary pin; rerun `vela id protect --user-presence --remove-source-key` with the installed release"
                .to_string()
        })?;
    let vela_binary_path =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let vela_binary_sha256 = vela_signer::contract::file_sha256(&vela_binary_path)?;
    if vela_binary_sha256 != format!("sha256:{}", pin.sha256) {
        return Err("running Vela binary differs from the protected ceremony pin".to_string());
    }
    let helper = crate::cli_identity::signer_helper_path(&vela_binary_path)?;
    let helper_sha256 = vela_signer::contract::file_sha256(&helper)?;
    if helper_sha256 != signer.helper_sha256 {
        return Err(format!(
            "installed signer helper {helper_sha256} does not match protected identity pin {}; rerun `vela id protect --user-presence --remove-source-key`",
            signer.helper_sha256
        ));
    }
    let runtime = BindRuntime {
        administrator_actor: identity.actor_id,
        administrator_public_key: identity.pubkey,
        vela_binary_path,
        vela_binary_sha256,
        helper_sha256,
        signer_provider: signer.provider,
        protection_grade: signer.protection_grade,
        protection_mode: signer.mode,
    };
    validate_runtime(&runtime)?;
    Ok(runtime)
}

fn validate_runtime(runtime: &BindRuntime) -> Result<(), String> {
    if !(runtime.administrator_actor.starts_with("reviewer:")
        || runtime.administrator_actor.starts_with("steward:"))
    {
        return Err("frontier bind runtime requires a reviewer: or steward: human".to_string());
    }
    if runtime.administrator_public_key.len() != 64
        || !runtime
            .administrator_public_key
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("frontier bind runtime administrator key is not Ed25519 hex".to_string());
    }
    for (name, digest) in [
        ("Vela binary", runtime.vela_binary_sha256.as_str()),
        ("signer helper", runtime.helper_sha256.as_str()),
    ] {
        let Some(hex) = digest.strip_prefix("sha256:") else {
            return Err(format!("{name} digest must use sha256:<64 lowercase hex>"));
        };
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!("{name} digest must use sha256:<64 lowercase hex>"));
        }
    }
    if vela_signer::contract::file_sha256(&runtime.vela_binary_path)? != runtime.vela_binary_sha256
    {
        return Err("frontier bind runtime Vela binary digest mismatch".to_string());
    }
    if runtime.signer_provider != "os_store"
        || !matches!(
            runtime.protection_grade.as_str(),
            "user_session" | "app_isolated" | "external_confirmed" | "hardware_nonexportable"
        )
    {
        return Err(
            "frontier bind requires a user-presence protected operating-system signer".to_string(),
        );
    }
    Ok(())
}

fn prepare_bind(
    frontier: &Path,
    reason: &str,
    observed_at: &str,
    runtime: &BindRuntime,
) -> Result<RepositoryBindPlan, String> {
    validate_runtime(runtime)?;
    if reason.trim().is_empty() || reason != reason.trim() {
        return Err(
            "frontier bind reason must be non-empty and have no outer whitespace".to_string(),
        );
    }
    chrono::DateTime::parse_from_rfc3339(observed_at)
        .map_err(|error| format!("frontier bind observation time must be RFC3339: {error}"))?;
    let frontier = std::fs::canonicalize(frontier)
        .map_err(|error| format!("resolve Frontier repository: {error}"))?;
    let (git_commit, git_tree) = assert_clean_repository(&frontier)?;
    let profile = match read_repository_profile(&frontier)? {
        Some(FrontierProfileFile::V1(profile)) => profile,
        Some(FrontierProfileFile::LegacyV0_1(_)) | None => {
            return Err(
                "frontier bind requires vela.frontier-profile.v1; migrate legacy repositories first"
                    .to_string(),
            );
        }
    };
    profile.validate()?;
    let project = vela_protocol::repo::load_from_path(&frontier)?;
    let replay = vela_protocol::reducer::verify_replay(&project);
    if !replay.ok {
        return Err(format!(
            "frontier bind requires exact replay: {}",
            replay.diffs.join(" | ")
        ));
    }
    if project
        .events
        .iter()
        .any(|event| event.kind.as_str() == EVENT_KIND_FRONTIER_REPOSITORY_BOUND)
    {
        return Err(
            "frontier bind creates only the first native administrator boundary; one already exists"
                .to_string(),
        );
    }
    if project.actors.len() != 1 {
        return Err(
            "frontier bind requires the exact one-actor registry installed by protected actor bootstrap; actor-registry extension is not supported"
                .to_string(),
        );
    }
    let actor = vela_protocol::proposals::validate_human_reviewer_authority_at(
        &project,
        &runtime.administrator_actor,
        observed_at,
    )?;
    if actor.public_key != runtime.administrator_public_key {
        return Err(
            "protected identity public key does not match the registered administrator actor"
                .to_string(),
        );
    }
    let authority = EffectiveFrontierAuthorityV1::from_events(&project.events)?;
    profile.assert_frontier_id(&authority.frontier_id)?;
    let projection = profile.project(&project)?;
    if authority.identity_event_root != projection.identity_event_root
        || authority.dependencies != projection.dependencies
    {
        return Err("frontier authority projection is internally inconsistent".to_string());
    }
    let parent = project
        .events
        .iter()
        .filter(|event| {
            vela_protocol::frontier_repository::repository_identity_event_content_root(event)
                .is_ok_and(|root| root == authority.identity_event_root)
        })
        .collect::<Vec<_>>();
    let [parent] = parent.as_slice() else {
        return Err(format!(
            "native repository identity parent is ambiguous or absent, found {}",
            parent.len()
        ));
    };
    if parent.kind.as_str() != "frontier.created" {
        return Err(
            "the first native administrator boundary must descend directly from frontier.created"
                .to_string(),
        );
    }
    let dependency_root = exact_dependency_root(&authority.dependencies)?;
    if dependency_root != authority.dependency_root {
        return Err(
            "effective dependency root does not match its exact dependency set".to_string(),
        );
    }
    let anchor =
        vela_edge::frontier_repository::derive_repository_anchor_facts(&frontier, &git_commit)?;
    if anchor.git_tree != git_tree {
        return Err("Git anchor tree changed during repository bind planning".to_string());
    }
    let payload = FrontierRepositoryBoundaryPayloadV1 {
        schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
        mode: FrontierRepositoryBoundaryMode::UpdateDependencies,
        frontier_id: authority.frontier_id.clone(),
        identity_root: authority.identity_root.clone(),
        observed_profile_root: projection.profile_root.clone(),
        dependency_root: dependency_root.clone(),
        dependencies: authority.dependencies.clone(),
        previous_identity_event_root: Some(authority.identity_event_root.clone()),
        legacy_identity_preimage_root: None,
        administrator_actor_id: actor.id.clone(),
        administrator_public_key: actor.public_key.clone(),
        administrator_algorithm: actor.algorithm.clone(),
        trust_mode: FrontierRepositoryTrustMode::Genesis,
        git_object_format: anchor.git_object_format,
        anchor_git_commit: anchor.git_commit.clone(),
        anchor_git_tree: anchor.git_tree.clone(),
        anchor_event_log_root: anchor.event_log_root.clone(),
        anchor_event_count: anchor.event_count,
        anchor_snapshot_root: anchor.snapshot_root,
        anchor_snapshot_schema: anchor.snapshot_schema,
        anchor_proposal_root: anchor.proposal_root,
        anchor_actor_registry_root: anchor.actor_registry_root,
        anchor_artifact_registry_root: anchor.artifact_registry_root,
        anchor_canonical_store_root: anchor.canonical_store_root,
    };
    let boundary_event = new_repository_boundary_event(payload, reason, observed_at)?;
    let boundary_event_content_root = unsigned_event_content_root(&boundary_event);
    let trust_anchor = RepositoryTrustAnchorV1 {
        schema: REPOSITORY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
        frontier_id: authority.frontier_id.clone(),
        identity_root: authority.identity_root.clone(),
        boundary_content_root: boundary_event_content_root.clone(),
        administrator_actor_id: actor.id.clone(),
        administrator_public_key: actor.public_key.clone(),
    };
    let trust_anchor_root = trust_anchor.root()?;
    let mut after: vela_protocol::project::Project =
        serde_json::from_value(serde_json::to_value(&project).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    after.events.push(boundary_event.clone());
    let event_count_after =
        u64::try_from(after.events.len()).map_err(|_| "event count exceeds u64".to_string())?;
    let mut plan = RepositoryBindPlan {
        schema: BIND_PLAN_SCHEMA.to_string(),
        ok: true,
        command: "frontier.bind".to_string(),
        frontier: frontier.display().to_string(),
        frontier_id: authority.frontier_id,
        frontier_name: profile.name,
        profile_root: projection.profile_root,
        identity_root: authority.identity_root,
        previous_identity_event_root: authority.identity_event_root,
        dependency_root,
        dependency_count: authority.dependencies.len(),
        administrator_actor: actor.id,
        administrator_public_key: actor.public_key,
        reason: reason.to_string(),
        observed_at: observed_at.to_string(),
        git_commit,
        git_tree,
        event_log_root_before: anchor.event_log_root,
        event_count_before: anchor.event_count,
        event_log_root_after: format!("sha256:{}", event_log_hash(&after.events)),
        event_count_after,
        boundary_event,
        boundary_event_content_root,
        trust_anchor,
        trust_anchor_root,
        vela_binary_path: runtime.vela_binary_path.display().to_string(),
        vela_binary_sha256: runtime.vela_binary_sha256.clone(),
        helper_sha256: runtime.helper_sha256.clone(),
        signer_provider: runtime.signer_provider.clone(),
        protection_grade: runtime.protection_grade.clone(),
        protection_mode: runtime.protection_mode,
        plan_root: String::new(),
    };
    plan.plan_root = plan_root(&plan)?;
    verify_plan(&plan)?;
    Ok(plan)
}

fn validate_confirmation(plan: &RepositoryBindPlan, confirmed_root: &str) -> Result<(), String> {
    verify_plan(plan)?;
    if plan.plan_root != confirmed_root {
        return Err(format!(
            "frontier bind confirmation root mismatch: supplied {confirmed_root}, current {}; no protected key was requested",
            plan.plan_root
        ));
    }
    Ok(())
}

fn build_signer_request(
    plan: &RepositoryBindPlan,
) -> Result<vela_signer::RepositoryBoundarySignerRequest, String> {
    use rand::RngCore;

    verify_plan(plan)?;
    let mut nonce = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let now = chrono::Utc::now();
    let request = vela_signer::RepositoryBoundarySignerRequest {
        schema: vela_signer::REPOSITORY_REQUEST_SCHEMA.to_string(),
        nonce: hex::encode(nonce),
        expires_at: (now + chrono::Duration::seconds(120))
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        vela_binary_path: plan.vela_binary_path.clone(),
        vela_binary_sha256: plan.vela_binary_sha256.clone(),
        helper_sha256: plan.helper_sha256.clone(),
        frontier_id: plan.frontier_id.clone(),
        frontier_path: plan.frontier.clone(),
        reason: plan.reason.clone(),
        administrator_actor: plan.administrator_actor.clone(),
        administrator_public_key: plan.administrator_public_key.clone(),
        observed_at: plan.observed_at.clone(),
        boundary_plan_root: plan.plan_root.clone(),
        provider: plan.signer_provider.clone(),
        protection_grade: plan.protection_grade.clone(),
        protection_mode: plan.protection_mode,
        display: vela_signer::RepositoryBoundaryDisplay {
            frontier_name: plan.frontier_name.clone(),
            profile_version: FRONTIER_PROFILE_SCHEMA_V1.to_string(),
            dependency_summary: format!(
                "{} exact dependencies · {}",
                plan.dependency_count, plan.dependency_root
            ),
            consequence: concat!(
                "update exact dependencies; first administrator boundary requires an out-of-band pin; ",
                "append one signed non-scientific repository boundary and install its exact local trust pin"
            )
            .to_string(),
        },
        event: plan.boundary_event.clone(),
    };
    vela_signer::validate_repository_boundary_request(&request, now)?;
    Ok(request)
}

fn request_protected_signature(
    request: &vela_signer::RepositoryBoundarySignerRequest,
) -> Result<vela_signer::RepositoryBoundarySignerResponse, String> {
    let helper = PathBuf::from(&request.vela_binary_path)
        .parent()
        .ok_or_else(|| "running Vela binary has no parent directory".to_string())?
        .join(if cfg!(target_os = "windows") {
            "vela-signer.exe"
        } else {
            "vela-signer"
        });
    let bytes = serde_json::to_vec(request)
        .map_err(|error| format!("encode repository-boundary signer request: {error}"))?;
    let mut child = Command::new(&helper)
        .arg("approve-repository-boundary")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start pinned signer helper {}: {error}", helper.display()))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "signer helper stdin is unavailable".to_string())?
        .write_all(&bytes)
        .map_err(|error| format!("write repository-boundary signer request: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for repository-boundary signer helper: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "signer helper declined or failed: {}",
            crate::cli::safe_text::inline(String::from_utf8_lossy(&output.stderr).trim())
        ));
    }
    let response: vela_signer::RepositoryBoundarySignerResponse =
        serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("decode repository-boundary signer response: {error}"))?;
    vela_signer::validate_repository_boundary_response(request, &response)?;
    Ok(response)
}

fn preflight_trust_install(
    trust_home: &Path,
    anchor: &RepositoryTrustAnchorV1,
) -> Result<(), String> {
    if let Some(existing) = load_repository_trust_anchor_from_home(trust_home, &anchor.frontier_id)?
        && existing.anchor != *anchor
    {
        return Err(format!(
            "refusing protected bind because trust anchor {} already pins a different first boundary",
            existing.path.display()
        ));
    }
    Ok(())
}

fn signed_event(
    request: &vela_signer::RepositoryBoundarySignerRequest,
    response: &vela_signer::RepositoryBoundarySignerResponse,
) -> Result<StateEvent, String> {
    vela_signer::validate_repository_boundary_request_fresh(request, chrono::Utc::now())?;
    vela_signer::validate_repository_boundary_response(request, response)?;
    let mut event = request.event.clone();
    event.signature = Some(response.event_signature.clone());
    verify_repository_boundary_signature_only(&event, &request.administrator_public_key)?;
    Ok(event)
}

fn execute_confirmed_bind_with_signer<F>(
    frontier: &Path,
    plan: &RepositoryBindPlan,
    runtime: &BindRuntime,
    trust_home: &Path,
    signer: F,
) -> Result<RepositoryBindResult, String>
where
    F: FnOnce(
        &vela_signer::RepositoryBoundarySignerRequest,
    ) -> Result<vela_signer::RepositoryBoundarySignerResponse, String>,
{
    verify_plan(plan)?;
    let frontier = std::fs::canonicalize(frontier)
        .map_err(|error| format!("resolve Frontier repository: {error}"))?;
    if frontier != Path::new(&plan.frontier) {
        return Err("confirmed repository bind plan names a different Frontier path".to_string());
    }
    preflight_trust_install(trust_home, &plan.trust_anchor)?;
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(&frontier)?;
    let barrier =
        FrontierTxn::acquire_first_administrator_boundary_barrier(&frontier, &journal_dir)
            .map_err(|error| error.to_string())?;

    // Rebuild the complete key-free plan while holding the recovery lock. No
    // protected prompt is shown if the checkout, actor registry, profile, Git
    // anchor, binary, helper, or requested semantics drifted.
    let locked = prepare_bind(&frontier, &plan.reason, &plan.observed_at, runtime)?;
    if !same_plan(&locked, plan)? {
        return Err(
            "repository bind plan drifted before protected user presence; rerun the preview"
                .to_string(),
        );
    }
    let request = build_signer_request(plan)?;
    let response = signer(&request)?;
    let event = signed_event(&request, &response)?;
    if repository_boundary_event_content_root(&event)? != plan.boundary_event_content_root {
        return Err(
            "protected signature returned a boundary with a different full content root"
                .to_string(),
        );
    }

    // A non-Vela writer can still change files while the Vela recovery lock is
    // held. Re-derive the same clean plan after user presence and before any
    // transaction journal or canonical postimage is prepared.
    let after_prompt = prepare_bind(&frontier, &plan.reason, &plan.observed_at, runtime)?;
    if !same_plan(&after_prompt, plan)? {
        return Err(
            "repository bind inputs drifted during protected user presence; no boundary was written"
                .to_string(),
        );
    }
    preflight_trust_install(trust_home, &plan.trust_anchor)?;

    let before = vela_protocol::repo::load_from_path(&frontier)?;
    if format!("sha256:{}", event_log_hash(&before.events)) != plan.event_log_root_before
        || before.events.len() as u64 != plan.event_count_before
    {
        return Err("repository event log drifted before bind transaction preparation".to_string());
    }
    let mut after: vela_protocol::project::Project =
        serde_json::from_value(serde_json::to_value(&before).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    after.events.push(event.clone());
    if format!("sha256:{}", event_log_hash(&after.events)) != plan.event_log_root_after
        || after.events.len() as u64 != plan.event_count_after
    {
        return Err("signed boundary does not produce the confirmed event-log result".to_string());
    }
    let managed = vela_protocol::repo::render_vela_repo_files(&frontier, &after)?;
    let writes = PlannedWrite::from_managed_files(managed).map_err(|error| error.to_string())?;
    let draft = DeltaDraft::prepare(&frontier, writes).map_err(|error| error.to_string())?;
    let canonical_delta_root = draft.delta.root().as_str().to_string();
    let publication_paths = draft
        .delta
        .public_writes()
        .map(|write| write.path.as_str().to_string())
        .collect::<Vec<_>>();
    let layout = vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
        "schema": "vela.frontier-layout.internal.v1",
        "frontier_id": plan.frontier_id,
        "paths": draft
            .delta
            .writes()
            .iter()
            .map(|write| write.path.as_str())
            .collect::<Vec<_>>(),
    }))?;
    let mut resulting_event_ids = after
        .events
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect::<Vec<_>>();
    resulting_event_ids.sort();
    resulting_event_ids.dedup();
    if resulting_event_ids.len() != after.events.len() {
        return Err("repository bind would create a duplicate event identifier".to_string());
    }
    let operation_id = OperationId::derive("frontier-bind", plan.plan_root.as_bytes());
    let result = serde_json::json!({
        "schema": BIND_RESULT_SCHEMA,
        "plan_root": plan.plan_root,
        "boundary_event_id": event.id,
        "boundary_event_content_root": plan.boundary_event_content_root,
        "trust_anchor_root": plan.trust_anchor_root,
        "canonical_delta_root": canonical_delta_root,
    });
    let transaction_plan = FrontierTxnPlan::new(
        FrontierTxnPlanSpec {
            kind: OperationKind::Maintenance,
            operation_id: operation_id.clone(),
            request_root: ContentDigest::parse(plan.plan_root.clone())
                .map_err(|error| error.to_string())?,
            frontier: FrontierBinding::new(&frontier, plan.frontier_id.clone(), &layout)
                .map_err(|error| error.to_string())?,
            fixed_time: plan.observed_at.clone(),
            expected_event_log_root: ContentDigest::parse(plan.event_log_root_before.clone())
                .map_err(|error| error.to_string())?,
            resulting_event_log_root: ContentDigest::parse(plan.event_log_root_after.clone())
                .map_err(|error| error.to_string())?,
            resulting_event_ids,
            read_set: vec![
                InputBinding::project_snapshot(&before).map_err(|error| error.to_string())?,
            ],
            result,
        },
        draft.delta.clone(),
    )
    .map_err(|error| error.to_string())?;
    let mut transaction = FrontierTxn::prepare_with_barrier(barrier, transaction_plan, draft)
        .map_err(|error| error.to_string())?;
    transaction
        .mark_committed()
        .map_err(|error| error.to_string())?;
    transaction.install().map_err(|error| error.to_string())?;
    transaction.complete().map_err(|error| error.to_string())?;

    let installed = install_repository_trust_anchor_from_home(trust_home, &plan.trust_anchor)
        .map_err(|error| {
            format!(
                "repository boundary committed, but the exact local trust pin was not installed: {error}; recover with `vela frontier trust pin {} --boundary-root {} --json`",
                frontier.display(),
                plan.boundary_event_content_root
            )
        })?;
    if installed.root != plan.trust_anchor_root || installed.anchor != plan.trust_anchor {
        return Err(
            "installed trust anchor differs from the confirmed repository bind plan".to_string(),
        );
    }

    let bound = vela_protocol::repo::load_from_path(&frontier)?;
    if bound.events.len() as u64 != plan.event_count_after
        || format!("sha256:{}", event_log_hash(&bound.events)) != plan.event_log_root_after
    {
        return Err(
            "completed repository bind event log differs from the confirmed plan".to_string(),
        );
    }
    let replay = vela_protocol::reducer::verify_replay(&bound);
    if !replay.ok {
        return Err(format!(
            "repository bind completed but replay failed: {}",
            replay.diffs.join(" | ")
        ));
    }
    let context = verify_repository_for_write(&frontier, &bound, Some(&installed.anchor))
        .map_err(|error| error.to_string())?;
    if context.profile.identity_event_root != plan.boundary_event_content_root {
        return Err(
            "completed repository bind did not become the effective identity head".to_string(),
        );
    }
    let publication = manual_uncommitted_exact_delta(
        &frontier,
        operation_id.as_str(),
        &canonical_delta_root,
        &publication_paths,
    );
    Ok(RepositoryBindResult {
        schema: BIND_RESULT_SCHEMA.to_string(),
        ok: true,
        command: "frontier.bind".to_string(),
        plan_root: plan.plan_root.clone(),
        operation_id: operation_id.as_str().to_string(),
        canonical_delta_root,
        event_id: event.id,
        boundary_event_content_root: plan.boundary_event_content_root.clone(),
        event_log_root: plan.event_log_root_after.clone(),
        event_count: plan.event_count_after,
        trust_anchor_root: installed.root,
        trust_anchor_path: installed.path.display().to_string(),
        publication,
        next_action: format!(
            "commit only the inspected canonical delta, then distribute first boundary root {} through an independent channel",
            plan.boundary_event_content_root
        ),
        replay_ok: true,
    })
}

pub(crate) fn cmd_frontier_bind(
    frontier: &Path,
    reason: &str,
    confirm_root: Option<&str>,
    confirm_at: Option<&str>,
    json: bool,
) {
    crate::ui::set_mode("frontier.bind", json);
    let runtime = production_runtime().unwrap_or_else(|error| super::fail_return(&error));
    match (confirm_root, confirm_at) {
        (None, None) => {
            let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let plan = prepare_bind(frontier, reason, &observed_at, &runtime)
                .unwrap_or_else(|error| super::fail_return(&error));
            if json {
                super::print_json(&plan);
            } else {
                println!("frontier bind · protected key-free preview");
                println!("  frontier: {}", plan.frontier_id);
                println!("  administrator: {}", plan.administrator_actor);
                println!(
                    "  dependencies: {} · {}",
                    plan.dependency_count, plan.dependency_root
                );
                println!("  first boundary: {}", plan.boundary_event_content_root);
                println!("  plan root: {}", plan.plan_root);
                println!("  confirm at: {}", plan.observed_at);
                println!("  writes now: none");
            }
        }
        (Some(confirm_root), Some(confirm_at)) => {
            crate::decision_plan::validate_scripted_confirmation_time(confirm_at).unwrap_or_else(
                |error| super::fail_return(&format!("{}: {}", error.code, error.message)),
            );
            let plan = prepare_bind(frontier, reason, confirm_at, &runtime)
                .unwrap_or_else(|error| super::fail_return(&error));
            validate_confirmation(&plan, confirm_root)
                .unwrap_or_else(|error| super::fail_return(&error));
            let trust_home = crate::frontier_txn::operating_system_account_home()
                .unwrap_or_else(|error| super::fail_return(&error.to_string()));
            let result = execute_confirmed_bind_with_signer(
                frontier,
                &plan,
                &runtime,
                &trust_home,
                request_protected_signature,
            )
            .unwrap_or_else(|error| super::fail_return(&error));
            if json {
                super::print_json(&result);
            } else {
                println!(
                    "{}",
                    repository_bind_result_human(&plan.frontier_id, &result)
                );
            }
        }
        _ => super::fail_return::<()>(
            "--confirm-root and --confirm-at must be supplied together; omit both for preview",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;
    use vela_protocol::frontier_repo::{ProfileV1InitOptions, initialize_profile_v1_minimal};
    use vela_protocol::sign::{ActorRecord, pubkey_hex, sign_event};

    const OBSERVED_AT: &str = "2026-07-22T16:00:00Z";

    fn git(path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    struct Fixture {
        directory: TempDir,
        key: SigningKey,
        runtime: BindRuntime,
    }

    fn fixture() -> Fixture {
        let directory = tempfile::tempdir().unwrap();
        initialize_profile_v1_minimal(
            directory.path(),
            ProfileV1InitOptions {
                name: "Native bind fixture",
                scope: "Can one protected first administrator boundary be exact?",
                initialize_git: true,
            },
        )
        .unwrap();
        git(directory.path(), &["config", "user.name", "Vela Test"]);
        git(
            directory.path(),
            &["config", "user.email", "vela@example.invalid"],
        );
        let key = SigningKey::from_bytes(&[61; 32]);
        let actor = ActorRecord {
            id: "reviewer:native-bind".to_string(),
            public_key: pubkey_hex(&key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T15:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        let mut project = vela_protocol::repo::load_from_path(directory.path()).unwrap();
        project.actors = vec![actor.clone()];
        vela_protocol::repo::save_to_path(directory.path(), &project).unwrap();
        git(directory.path(), &["add", "."]);
        git(
            directory.path(),
            &["commit", "-qm", "bootstrap human actor"],
        );
        let executable = std::env::current_exe().unwrap();
        let digest = vela_signer::contract::file_sha256(&executable).unwrap();
        Fixture {
            directory,
            key,
            runtime: BindRuntime {
                administrator_actor: actor.id,
                administrator_public_key: actor.public_key,
                vela_binary_path: executable,
                vela_binary_sha256: digest.clone(),
                helper_sha256: digest,
                signer_provider: "os_store".to_string(),
                protection_grade: "user_session".to_string(),
                protection_mode: vela_signer::ProtectionMode::Session,
            },
        }
    }

    fn fake_response(
        request: &vela_signer::RepositoryBoundarySignerRequest,
        key: &SigningKey,
    ) -> Result<vela_signer::RepositoryBoundarySignerResponse, String> {
        Ok(vela_signer::RepositoryBoundarySignerResponse {
            schema: vela_signer::REPOSITORY_RESPONSE_SCHEMA.to_string(),
            request_root: vela_signer::repository_boundary_request_root(request)?,
            administrator_public_key: request.administrator_public_key.clone(),
            helper_version: "test".to_string(),
            helper_sha256: request.helper_sha256.clone(),
            provider: request.provider.clone(),
            protection_grade: request.protection_grade.clone(),
            approved_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            protection_mode: request.protection_mode,
            event_id: request.event.id.clone(),
            event_signature: sign_event(&request.event, key)?,
        })
    }

    #[test]
    fn init_bootstrap_preview_and_protected_bind_install_exact_first_pin() {
        let fixture = fixture();
        let trust_home = tempfile::tempdir().unwrap();
        let before = vela_protocol::repo::load_from_path(fixture.directory.path()).unwrap();
        let plan = prepare_bind(
            fixture.directory.path(),
            "Bind the first exact administrator",
            OBSERVED_AT,
            &fixture.runtime,
        )
        .unwrap();
        assert_eq!(plan.dependency_count, 0);
        assert_eq!(plan.event_count_after, plan.event_count_before + 1);
        assert!(
            load_repository_trust_anchor_from_home(trust_home.path(), &plan.frontier_id)
                .unwrap()
                .is_none()
        );

        let result = execute_confirmed_bind_with_signer(
            fixture.directory.path(),
            &plan,
            &fixture.runtime,
            trust_home.path(),
            |request| fake_response(request, &fixture.key),
        )
        .unwrap();
        assert_eq!(
            result.boundary_event_content_root,
            plan.boundary_event_content_root
        );
        let installed =
            load_repository_trust_anchor_from_home(trust_home.path(), &plan.frontier_id)
                .unwrap()
                .unwrap();
        assert_eq!(installed.anchor, plan.trust_anchor);
        assert_eq!(
            installed.anchor.boundary_content_root,
            plan.boundary_event_content_root
        );
        assert_eq!(installed.root, plan.trust_anchor_root);
        let wire = serde_json::to_value(&result).unwrap();
        assert_eq!(wire["publication"]["state"], "uncommitted");
        assert!(
            wire.get("git_publication").is_none(),
            "legacy ad-hoc publication field survived: {wire}"
        );
        assert_eq!(wire["canonical_delta_root"], result.canonical_delta_root);
        let publication_reason = wire["publication"]["reason"].as_str().unwrap();
        assert!(publication_reason.contains(&result.operation_id));
        assert!(publication_reason.contains(&result.canonical_delta_root));
        let recovery = wire["publication"]["recovery_command"].as_str().unwrap();
        assert!(recovery.starts_with("git -C "));
        assert!(recovery.contains(" status --short -- "));
        assert!(recovery.contains(&format!(".vela/events/{}.json", result.event_id)));
        let human = repository_bind_result_human(&plan.frontier_id, &result);
        assert!(human.contains("Git publication: uncommitted"));
        assert!(human.contains(&result.operation_id));
        assert!(human.contains(&result.canonical_delta_root));
        assert!(human.contains(recovery));
        assert!(!human.contains("not performed"));

        let after = vela_protocol::repo::load_from_path(fixture.directory.path()).unwrap();
        assert_eq!(after.events.len(), before.events.len() + 1);
        assert_eq!(
            after
                .events
                .iter()
                .filter(|event| event.kind.as_str() == EVENT_KIND_FRONTIER_REPOSITORY_BOUND)
                .count(),
            1
        );
        for historical in &before.events {
            let retained = after
                .events
                .iter()
                .find(|event| event.id == historical.id)
                .expect("historical event remains present");
            assert_eq!(
                serde_json::to_value(retained).unwrap(),
                serde_json::to_value(historical).unwrap()
            );
        }
        git(fixture.directory.path(), &["add", "."]);
        git(
            fixture.directory.path(),
            &["commit", "-qm", "first administrator boundary"],
        );
        let existing = prepare_bind(
            fixture.directory.path(),
            "Bind another first administrator",
            OBSERVED_AT,
            &fixture.runtime,
        )
        .unwrap_err();
        assert!(existing.contains("one already exists"));
    }

    #[test]
    fn cancellation_and_drift_do_not_write_boundary_or_pin() {
        let fixture = fixture();
        let trust_home = tempfile::tempdir().unwrap();
        let plan = prepare_bind(
            fixture.directory.path(),
            "Bind the first exact administrator",
            OBSERVED_AT,
            &fixture.runtime,
        )
        .unwrap();
        let before = vela_protocol::repo::load_from_path(fixture.directory.path()).unwrap();
        let cancelled = execute_confirmed_bind_with_signer(
            fixture.directory.path(),
            &plan,
            &fixture.runtime,
            trust_home.path(),
            |_| Err("user cancelled protected approval".to_string()),
        );
        assert!(cancelled.unwrap_err().contains("cancelled"));
        let after_cancel = vela_protocol::repo::load_from_path(fixture.directory.path()).unwrap();
        assert_eq!(
            serde_json::to_value(&after_cancel.events).unwrap(),
            serde_json::to_value(&before.events).unwrap()
        );
        assert!(
            load_repository_trust_anchor_from_home(trust_home.path(), &plan.frontier_id)
                .unwrap()
                .is_none()
        );

        let drifted = execute_confirmed_bind_with_signer(
            fixture.directory.path(),
            &plan,
            &fixture.runtime,
            trust_home.path(),
            |request| {
                std::fs::write(
                    fixture.directory.path().join("README.md"),
                    "hostile drift\n",
                )
                .unwrap();
                fake_response(request, &fixture.key)
            },
        );
        assert!(drifted.unwrap_err().contains("clean checkout"));
        let after_drift = vela_protocol::repo::load_from_path(fixture.directory.path()).unwrap();
        assert_eq!(
            serde_json::to_value(&after_drift.events).unwrap(),
            serde_json::to_value(&before.events).unwrap()
        );
        assert!(
            load_repository_trust_anchor_from_home(trust_home.path(), &plan.frontier_id)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn rejects_legacy_nonhuman_existing_boundary_and_stale_plan() {
        let fixture = fixture();
        let mut nonhuman = fixture.runtime.clone();
        nonhuman.administrator_actor = "agent:not-human".to_string();
        let error = prepare_bind(
            fixture.directory.path(),
            "Bind the first exact administrator",
            OBSERVED_AT,
            &nonhuman,
        )
        .unwrap_err();
        assert!(error.contains("reviewer: or steward: human"));

        let mut plan = prepare_bind(
            fixture.directory.path(),
            "Bind the first exact administrator",
            OBSERVED_AT,
            &fixture.runtime,
        )
        .unwrap();
        plan.reason = "drifted reason".to_string();
        assert!(verify_plan(&plan).is_err());
        let current = prepare_bind(
            fixture.directory.path(),
            "Bind the first exact administrator",
            OBSERVED_AT,
            &fixture.runtime,
        )
        .unwrap();
        assert!(
            validate_confirmation(&current, &format!("sha256:{}", "f".repeat(64)))
                .unwrap_err()
                .contains("no protected key was requested")
        );
        let mut unprotected = fixture.runtime.clone();
        unprotected.signer_provider = "file".to_string();
        assert!(
            prepare_bind(
                fixture.directory.path(),
                "Bind the first exact administrator",
                OBSERVED_AT,
                &unprotected,
            )
            .unwrap_err()
            .contains("user-presence protected")
        );

        let legacy = tempfile::tempdir().unwrap();
        vela_protocol::frontier_repo::initialize_minimal(
            legacy.path(),
            vela_protocol::frontier_repo::InitOptions {
                name: "Legacy",
                initialize_git: true,
            },
        )
        .unwrap();
        git(legacy.path(), &["config", "user.name", "Vela Test"]);
        git(
            legacy.path(),
            &["config", "user.email", "vela@example.invalid"],
        );
        git(legacy.path(), &["add", "."]);
        git(legacy.path(), &["commit", "-qm", "legacy baseline"]);
        let legacy_error = prepare_bind(
            legacy.path(),
            "Bind the first exact administrator",
            OBSERVED_AT,
            &fixture.runtime,
        )
        .unwrap_err();
        assert!(
            legacy_error.contains("requires vela.frontier-profile.v1"),
            "{legacy_error}"
        );
    }

    #[test]
    fn rejects_extra_actor_before_requesting_a_protected_boundary() {
        let fixture = fixture();
        let mut project = vela_protocol::repo::load_from_path(fixture.directory.path()).unwrap();
        let injected_key = SigningKey::from_bytes(&[62; 32]);
        project.actors.push(ActorRecord {
            id: "agent:injected".to_string(),
            public_key: pubkey_hex(&injected_key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T15:01:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        vela_protocol::repo::save_to_path(fixture.directory.path(), &project).unwrap();
        git(fixture.directory.path(), &["add", "."]);
        git(
            fixture.directory.path(),
            &["commit", "-qm", "inject unsupported registry extension"],
        );

        let error = prepare_bind(
            fixture.directory.path(),
            "Bind the first exact administrator",
            OBSERVED_AT,
            &fixture.runtime,
        )
        .unwrap_err();
        assert!(error.contains("exact one-actor registry"), "{error}");
        assert!(error.contains("extension is not supported"), "{error}");
    }

    #[test]
    fn empty_registry_is_rejected_before_a_protected_request() {
        let directory = tempfile::tempdir().unwrap();
        initialize_profile_v1_minimal(
            directory.path(),
            ProfileV1InitOptions {
                name: "Empty registry",
                scope: "Can an empty registry bind an administrator?",
                initialize_git: true,
            },
        )
        .unwrap();
        git(directory.path(), &["config", "user.name", "Vela Test"]);
        git(
            directory.path(),
            &["config", "user.email", "vela@example.invalid"],
        );
        git(directory.path(), &["add", "."]);
        git(
            directory.path(),
            &["commit", "-qm", "empty registry baseline"],
        );
        let template = fixture();
        let error = prepare_bind(
            directory.path(),
            "Bind without a registered human",
            OBSERVED_AT,
            &template.runtime,
        )
        .unwrap_err();
        assert!(error.contains("exact one-actor registry"), "{error}");
        assert!(error.contains("protected actor bootstrap"), "{error}");
    }

    #[test]
    fn conflicting_preexisting_out_of_band_pin_fails_before_signer() {
        let fixture = fixture();
        let trust_home = tempfile::tempdir().unwrap();
        let plan = prepare_bind(
            fixture.directory.path(),
            "Bind the first exact administrator",
            OBSERVED_AT,
            &fixture.runtime,
        )
        .unwrap();
        let mut conflicting = plan.trust_anchor.clone();
        conflicting.boundary_content_root = sha256_root(b"different first boundary");
        install_repository_trust_anchor_from_home(trust_home.path(), &conflicting).unwrap();
        let mut called = false;
        let error = execute_confirmed_bind_with_signer(
            fixture.directory.path(),
            &plan,
            &fixture.runtime,
            trust_home.path(),
            |_| {
                called = true;
                Err("must not reach signer".to_string())
            },
        )
        .unwrap_err();
        assert!(error.contains("already pins a different first boundary"));
        assert!(!called);
    }

    #[test]
    fn conflicting_pin_created_during_user_presence_blocks_before_canonical_write() {
        let fixture = fixture();
        let trust_home = tempfile::tempdir().unwrap();
        let plan = prepare_bind(
            fixture.directory.path(),
            "Bind the first exact administrator",
            OBSERVED_AT,
            &fixture.runtime,
        )
        .unwrap();
        let before = vela_protocol::repo::load_from_path(fixture.directory.path()).unwrap();
        let mut conflicting = plan.trust_anchor.clone();
        conflicting.boundary_content_root = sha256_root(b"racing first boundary");
        let error = execute_confirmed_bind_with_signer(
            fixture.directory.path(),
            &plan,
            &fixture.runtime,
            trust_home.path(),
            |request| {
                install_repository_trust_anchor_from_home(trust_home.path(), &conflicting)?;
                fake_response(request, &fixture.key)
            },
        )
        .unwrap_err();
        assert!(error.contains("already pins a different first boundary"));
        let after = vela_protocol::repo::load_from_path(fixture.directory.path()).unwrap();
        assert_eq!(
            serde_json::to_value(after.events).unwrap(),
            serde_json::to_value(before.events).unwrap()
        );
        assert_eq!(
            load_repository_trust_anchor_from_home(trust_home.path(), &plan.frontier_id)
                .unwrap()
                .unwrap()
                .anchor,
            conflicting
        );
    }
}
