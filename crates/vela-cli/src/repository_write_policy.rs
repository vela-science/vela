//! CLI-owned repository write authorization.
//!
//! The repository transaction runtime carries one opaque, in-memory capability
//! and invokes it at two lifecycle boundaries. It does not know what makes a
//! fresh initialization, routine evidence write, or authority write valid.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::authority_trust::{
    authority_trust_anchor_path, load_authority_trust_anchor_from_home,
};
use serde::Serialize;

use vela_repository::{
    CanonicalWriteBarrier, ContentDigest, FileState, RepositoryRecoveryBarrier, RepositoryTxn,
    RepositoryTxnError, StagedWrite, TransactionAuthorization, TransactionAuthorizationContext,
    WriteClass,
};

const FRESH_CONTEXT_SCHEMA: &str = "vela.fresh-repository-write-context.internal.v1";
const ROUTINE_CONTEXT_SCHEMA: &str = "vela.routine-evidence-write-context.internal.v1";
const AUTHORITY_CONTEXT_SCHEMA: &str = "vela.repository-authority-write-context.internal.v1";

fn denied(intent: &'static str, reason: impl Into<String>) -> RepositoryTxnError {
    RepositoryTxnError::RepositoryWriteIntentDenied {
        intent,
        reason: reason.into(),
    }
}

const BOOTSTRAP_ABSENT_PATHS: &[&str] = &[
    ".vela/epoch.json",
    ".vela/origin.json",
    ".vela/repository.json",
    ".vela/authority",
    ".vela/claims",
    ".vela/proposals",
    ".vela/submissions",
    ".vela/verifications",
    ".vela/artifacts",
    "records",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct FreshContext {
    repository_id: String,
    profile_root: String,
    canonical_os_home: String,
    bootstrap_absence_root: String,
    trust_anchor_path: String,
    trust_anchor_absent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RepositoryContext {
    repository_id: String,
    repository_root: String,
    origin_id: String,
    origin_root: String,
    final_authority_record_root: String,
    final_authority_event_log_root: String,
    active_keyset_root: String,
    active_authorization_model_root: String,
    authority_event_count: usize,
    authority_record_count: usize,
    closed: bool,
    closure_event_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct AuthorityContext {
    repository: RepositoryContext,
    canonical_os_home: String,
    trust_anchor_path: String,
    trust_anchor_root: String,
    sequence_one_record_root: String,
    initialization_event_id: String,
    initialization_event_root: String,
}

#[derive(Serialize)]
struct PolicyCommitment<'a, T: ?Sized> {
    schema: &'static str,
    context: &'a T,
    plan_root: &'a str,
    delta_root: &'a str,
}

fn policy_commitment_root<T: Serialize + ?Sized>(
    schema: &'static str,
    context: &T,
    plan_root: &ContentDigest,
    delta_root: &ContentDigest,
) -> Result<ContentDigest, RepositoryTxnError> {
    let bytes = vela_protocol::canonical::to_canonical_bytes(&PolicyCommitment {
        schema,
        context,
        plan_root: plan_root.as_str(),
        delta_root: delta_root.as_str(),
    })
    .map_err(RepositoryTxnError::Canonicalize)?;
    Ok(ContentDigest::hash(bytes))
}

fn canonical_repository_path(path: &Path) -> Result<PathBuf, RepositoryTxnError> {
    let metadata = fs::metadata(path).map_err(|error| {
        RepositoryTxnError::Io(format!("read repository root {}: {error}", path.display()))
    })?;
    if !metadata.is_dir() {
        return Err(RepositoryTxnError::Io(format!(
            "repository root is not a directory: {}",
            path.display()
        )));
    }
    fs::canonicalize(path).map_err(|error| {
        RepositoryTxnError::Io(format!(
            "canonicalize repository root {}: {error}",
            path.display()
        ))
    })
}

fn canonical_account_home() -> Result<PathBuf, RepositoryTxnError> {
    let home = operating_system_account_home()?;
    fs::canonicalize(&home).map_err(|error| {
        RepositoryTxnError::WriteAuthorization(format!(
            "canonicalize operating-system account home {}: {error}",
            home.display()
        ))
    })
}

fn exact_path_string(path: &Path, label: &str) -> Result<String, RepositoryTxnError> {
    path.to_str().map(str::to_string).ok_or_else(|| {
        RepositoryTxnError::WriteAuthorization(format!(
            "{label} is not valid UTF-8 and cannot be committed exactly: {}",
            path.display()
        ))
    })
}

fn require_absent(path: &Path, intent: &'static str) -> Result<(), RepositoryTxnError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(denied(
            intent,
            format!("required absent path exists: {}", path.display()),
        )),
        Err(error) => Err(RepositoryTxnError::Io(format!(
            "inspect required absent path {}: {error}",
            path.display()
        ))),
    }
}

fn bootstrap_absence_root() -> Result<String, RepositoryTxnError> {
    let bytes = vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
        "schema": "vela.repository-bootstrap-absence.internal.v1",
        "absent_paths": BOOTSTRAP_ABSENT_PATHS,
    }))
    .map_err(RepositoryTxnError::Canonicalize)?;
    Ok(ContentDigest::hash(bytes).as_str().to_string())
}

fn fresh_context(root: &Path) -> Result<FreshContext, RepositoryTxnError> {
    let intent = "repository_authority_initialization";
    let root = canonical_repository_path(root)?;
    let trusted_user_home = canonical_account_home()?;
    for relative in BOOTSTRAP_ABSENT_PATHS {
        require_absent(&root.join(relative), intent)?;
    }
    let profile =
        crate::repository::verify_bootstrap_at(&root).map_err(|reason| denied(intent, reason))?;
    let profile_root = profile
        .profile_root()
        .map_err(|reason| denied(intent, reason))?;
    let trust_anchor_path = authority_trust_anchor_path(&trusted_user_home, &profile.repository_id)
        .map_err(RepositoryTxnError::WriteAuthorization)?;
    if load_authority_trust_anchor_from_home(&trusted_user_home, &profile.repository_id)
        .map_err(RepositoryTxnError::WriteAuthorization)?
        .is_some()
    {
        return Err(denied(
            intent,
            "fresh repository authority requires an absent local trust anchor",
        ));
    }
    Ok(FreshContext {
        repository_id: profile.repository_id,
        profile_root,
        canonical_os_home: exact_path_string(&trusted_user_home, "operating-system account home")?,
        bootstrap_absence_root: bootstrap_absence_root()?,
        trust_anchor_path: exact_path_string(&trust_anchor_path, "authority trust-anchor path")?,
        trust_anchor_absent: true,
    })
}

fn read_repository_origin(
    root: &Path,
    intent: &'static str,
) -> Result<vela_protocol::repository_origin::RepositoryOriginV1, RepositoryTxnError> {
    let bytes = fs::read(root.join(".vela/origin.json"))
        .map_err(|error| RepositoryTxnError::Io(format!("read repository origin: {error}")))?;
    vela_protocol::repository_origin::RepositoryOriginV1::parse(&bytes)
        .map_err(|error| denied(intent, format!("repository origin is invalid: {error}")))
}

fn load_repository_state(
    root: &Path,
    intent: &'static str,
) -> Result<
    (
        PathBuf,
        vela_protocol::repository::RepositoryV4,
        vela_protocol::repository_origin::RepositoryOriginV1,
        crate::cli::LoadedRepositoryAuthority,
    ),
    RepositoryTxnError,
> {
    let root = canonical_repository_path(root)?;
    let repository = crate::repository::verify_repository_at(&root, true)
        .map_err(|error| denied(intent, format!("repository origin is invalid: {error}")))?;
    let origin = read_repository_origin(&root, intent)?;
    let authority =
        crate::cli::load_repository_authority(&root, &repository, &origin).map_err(|error| {
            denied(
                intent,
                format!("current repository-authority history is invalid: {error}"),
            )
        })?;
    Ok((root, repository, origin, authority))
}

fn routine_context(root: &Path) -> Result<RepositoryContext, RepositoryTxnError> {
    let intent = "routine_evidence";
    let (_, repository, origin, authority) = load_repository_state(root, intent)?;
    repository_context_from_verified(&repository, &origin, &authority, intent)
}

fn required_authority_root(
    intent: &'static str,
    name: &str,
    root: Option<&str>,
) -> Result<String, RepositoryTxnError> {
    root.map(str::to_string).ok_or_else(|| {
        denied(
            intent,
            format!("current repository-authority history has no {name}"),
        )
    })
}

fn repository_context_from_verified(
    repository: &vela_protocol::repository::RepositoryV4,
    origin: &vela_protocol::repository_origin::RepositoryOriginV1,
    authority: &crate::cli::LoadedRepositoryAuthority,
    intent: &'static str,
) -> Result<RepositoryContext, RepositoryTxnError> {
    repository
        .verify()
        .map_err(|reason| denied(intent, reason))?;
    if authority.verification.closed {
        return Err(denied(intent, "repository-authority history is closed"));
    }
    let origin_id = origin.id().map_err(RepositoryTxnError::CorruptPlan)?;
    let origin_root = origin
        .canonical_root()
        .map_err(RepositoryTxnError::CorruptPlan)?;
    if repository.repository_id != origin.repository_id
        || repository.profile_root != origin.profile_root
        || repository.origin_id != origin_id
        || repository.origin_root != origin_root
    {
        return Err(denied(
            intent,
            "repository manifest does not bind the exact current origin",
        ));
    }
    Ok(RepositoryContext {
        repository_id: repository.repository_id.clone(),
        repository_root: repository
            .canonical_root()
            .map_err(RepositoryTxnError::CorruptPlan)?,
        origin_id,
        origin_root,
        final_authority_record_root: required_authority_root(
            intent,
            "head record",
            authority
                .verification
                .final_authority_record_root
                .as_deref(),
        )?,
        final_authority_event_log_root: authority.verification.final_event_log_root.clone(),
        active_keyset_root: required_authority_root(
            intent,
            "active keyset",
            authority
                .verification
                .final_authority_keyset_root
                .as_deref(),
        )?,
        active_authorization_model_root: required_authority_root(
            intent,
            "active authorization model",
            authority
                .verification
                .final_authorization_model_root
                .as_deref(),
        )?,
        authority_event_count: authority.verification.authority_event_count,
        authority_record_count: authority.verification.authority_record_count,
        closed: false,
        closure_event_id: authority.verification.closure_event_id.clone(),
    })
}

fn authority_context_from_verified(
    repository: &vela_protocol::repository::RepositoryV4,
    origin: &vela_protocol::repository_origin::RepositoryOriginV1,
    authority: &crate::cli::LoadedRepositoryAuthority,
) -> Result<AuthorityContext, RepositoryTxnError> {
    let intent = "repository_authority";
    let repository_context =
        repository_context_from_verified(repository, origin, authority, intent)?;
    let sequence_one_record_root = required_authority_root(
        intent,
        "sequence-one record root",
        authority
            .verification
            .first_authority_record_root
            .as_deref(),
    )?;
    let trusted_user_home = canonical_account_home()?;
    let anchor = load_authority_trust_anchor_from_home(
        &trusted_user_home,
        &repository.repository_id,
    )
    .map_err(|error| denied(intent, format!("load local authority trust anchor: {error}")))?
    .ok_or_else(|| {
        denied(
            intent,
            format!(
            "current repository-authority writes require an independent sequence-one pin; run `vela authority trust pin . --record-root {sequence_one_record_root} --json`"
            ),
        )
    })?;
    anchor
        .anchor
        .verify_sequence_one(&repository.repository_id, &sequence_one_record_root)
        .map_err(|_| {
            denied(
                intent,
                format!(
                "local authority trust anchor does not select current sequence one {sequence_one_record_root}"
                ),
            )
        })?;
    let initialization_event_id = authority
        .verification
        .initialization_event_id
        .as_deref()
        .ok_or_else(|| {
            denied(
                intent,
                "current repository authority has no origin initialization event",
            )
        })?;
    let initialization_event = authority
        .history
        .authority_events
        .iter()
        .find(|event| event.id == initialization_event_id)
        .ok_or_else(|| {
            denied(
                intent,
                format!("current origin initialization event {initialization_event_id} is missing"),
            )
        })?;
    Ok(AuthorityContext {
        repository: repository_context,
        canonical_os_home: exact_path_string(&trusted_user_home, "operating-system account home")?,
        trust_anchor_path: exact_path_string(&anchor.path, "authority trust-anchor path")?,
        trust_anchor_root: anchor.root,
        sequence_one_record_root,
        initialization_event_id: initialization_event_id.to_string(),
        initialization_event_root: initialization_event
            .root()
            .map_err(RepositoryTxnError::CorruptPlan)?,
    })
}

fn authority_context(root: &Path) -> Result<AuthorityContext, RepositoryTxnError> {
    let intent = "repository_authority";
    let (_, repository, origin, authority) = load_repository_state(root, intent)?;
    authority_context_from_verified(&repository, &origin, &authority)
}

fn verify_binding(
    context: &TransactionAuthorizationContext<'_>,
    expected_repository_id: &str,
) -> Result<(), RepositoryTxnError> {
    let binding = context.repository_binding();
    if binding.repository_id() != expected_repository_id {
        return Err(RepositoryTxnError::WriteAuthorizationRepositoryMismatch {
            authorized: expected_repository_id.to_string(),
            planned: binding.repository_id().to_string(),
        });
    }
    Ok(())
}

#[derive(Debug)]
enum PolicyContext {
    Fresh(FreshContext),
    Routine(RepositoryContext),
    Authority(AuthorityContext),
}

impl PolicyContext {
    fn repository_id(&self) -> &str {
        match self {
            Self::Fresh(context) => &context.repository_id,
            Self::Routine(context) => &context.repository_id,
            Self::Authority(context) => &context.repository.repository_id,
        }
    }

    fn reload(&self, root: &Path) -> Result<Self, RepositoryTxnError> {
        match self {
            Self::Fresh(_) => fresh_context(root).map(Self::Fresh),
            Self::Routine(_) => routine_context(root).map(Self::Routine),
            Self::Authority(_) => authority_context(root).map(Self::Authority),
        }
    }

    fn verify_delta(
        &self,
        transaction: &mut TransactionAuthorizationContext<'_>,
    ) -> Result<(), RepositoryTxnError> {
        match self {
            Self::Fresh(context) => verify_fresh_repository_delta(transaction, context),
            Self::Routine(_) | Self::Authority(_) => Ok(()),
        }
    }

    fn commitment(
        &self,
        transaction: &TransactionAuthorizationContext<'_>,
    ) -> Result<ContentDigest, RepositoryTxnError> {
        let plan_root = transaction.plan_root();
        let delta_root = transaction.canonical_delta().root();
        match self {
            Self::Fresh(context) => {
                policy_commitment_root(FRESH_CONTEXT_SCHEMA, context, plan_root, delta_root)
            }
            Self::Routine(context) => {
                policy_commitment_root(ROUTINE_CONTEXT_SCHEMA, context, plan_root, delta_root)
            }
            Self::Authority(context) => {
                policy_commitment_root(AUTHORITY_CONTEXT_SCHEMA, context, plan_root, delta_root)
            }
        }
    }
}

#[derive(Debug)]
struct AuthorizationBinding {
    commitment: ContentDigest,
    delta_root: ContentDigest,
}

#[derive(Debug)]
struct RepositoryWriteAuthorization {
    context: PolicyContext,
    binding: Option<AuthorizationBinding>,
}

impl RepositoryWriteAuthorization {
    fn validate(
        &self,
        transaction: &mut TransactionAuthorizationContext<'_>,
    ) -> Result<ContentDigest, RepositoryTxnError> {
        verify_binding(transaction, self.context.repository_id())?;
        let actual = self.context.reload(transaction.repository_root())?;
        self.context.verify_delta(transaction)?;
        let expected = self.context.commitment(transaction)?;
        let actual = actual.commitment(transaction)?;
        if actual != expected {
            return Err(RepositoryTxnError::StaleWriteAuthorization { expected, actual });
        }
        Ok(expected)
    }
}

impl TransactionAuthorization for RepositoryWriteAuthorization {
    fn bind_plan(
        &mut self,
        transaction: &mut TransactionAuthorizationContext<'_>,
    ) -> Result<(), RepositoryTxnError> {
        let candidate = self.validate(transaction)?;
        let delta_root = transaction.canonical_delta().root().clone();
        if let Some(bound) = &self.binding {
            if bound.delta_root != delta_root {
                return Err(RepositoryTxnError::WriteAuthorizationDeltaMismatch {
                    authorized: bound.delta_root.clone(),
                    planned: delta_root,
                });
            }
            if bound.commitment != candidate {
                return Err(RepositoryTxnError::StaleWriteAuthorization {
                    expected: bound.commitment.clone(),
                    actual: candidate,
                });
            }
        } else {
            self.binding = Some(AuthorizationBinding {
                commitment: candidate,
                delta_root,
            });
        }
        Ok(())
    }

    fn revalidate_for_marker(
        &self,
        transaction: &mut TransactionAuthorizationContext<'_>,
    ) -> Result<(), RepositoryTxnError> {
        let actual = self.validate(transaction)?;
        let expected = self
            .binding
            .as_ref()
            .ok_or_else(|| RepositoryTxnError::WriteAuthorization("unbound capability".into()))?;
        let delta_root = transaction.canonical_delta().root();
        if expected.delta_root != *delta_root {
            return Err(RepositoryTxnError::WriteAuthorizationDeltaMismatch {
                authorized: expected.delta_root.clone(),
                planned: delta_root.clone(),
            });
        }
        if expected.commitment != actual {
            return Err(RepositoryTxnError::StaleWriteAuthorization {
                expected: expected.commitment.clone(),
                actual,
            });
        }
        Ok(())
    }
}

fn staged_postimage(
    transaction: &mut TransactionAuthorizationContext<'_>,
    write: &StagedWrite,
) -> Result<Vec<u8>, RepositoryTxnError> {
    transaction.postimage_bytes(write)?.ok_or_else(|| {
        RepositoryTxnError::CorruptPlan(format!(
            "write {} has no file postimage",
            write.path.as_str()
        ))
    })
}

fn staged_authority_event(
    transaction: &mut TransactionAuthorizationContext<'_>,
    write: &StagedWrite,
) -> Result<Option<vela_protocol::authority::AuthorityEventV1>, RepositoryTxnError> {
    let Some(relative) = write.path.as_str().strip_prefix(".vela/authority/events/") else {
        return Ok(None);
    };
    let Some(event_id) = relative.strip_suffix(".json") else {
        return Err(denied(
            "repository_authority_initialization",
            format!(
                "authority event write {} is not one direct JSON event",
                write.path.as_str()
            ),
        ));
    };
    if write.class != WriteClass::Authority
        || !matches!(write.preimage, FileState::Absent)
        || !matches!(write.postimage, FileState::File { .. })
    {
        return Err(denied(
            "repository_authority_initialization",
            format!(
                "authority events are append-only Authority writes: {}",
                write.path.as_str()
            ),
        ));
    }
    let bytes = staged_postimage(transaction, write)?;
    let event = serde_json::from_slice::<vela_protocol::authority::AuthorityEventV1>(&bytes)
        .map_err(|error| {
            RepositoryTxnError::CorruptPlan(format!(
                "authority event write {} is invalid: {error}",
                write.path.as_str()
            ))
        })?;
    event.validate().map_err(RepositoryTxnError::CorruptPlan)?;
    if event.id != event_id {
        return Err(RepositoryTxnError::CorruptPlan(format!(
            "authority event write {} has a mismatched content id",
            write.path.as_str()
        )));
    }
    Ok(Some(event))
}

fn verify_fresh_repository_delta(
    transaction: &mut TransactionAuthorizationContext<'_>,
    expected: &FreshContext,
) -> Result<(), RepositoryTxnError> {
    let deny = |reason: String| RepositoryTxnError::RepositoryWriteIntentDenied {
        intent: "repository_authority_initialization",
        reason,
    };
    let writes = transaction.canonical_delta().writes().to_vec();
    let mut authority_initializations = 0_usize;
    let mut authority_records = 0_usize;
    let mut repository_origins = Vec::new();
    let mut repository_manifests = Vec::new();

    for write in &writes {
        let path = write.path.as_str();
        if path.starts_with(".vela/events/") {
            return Err(deny(
                "fresh repository authority cannot append a retired event".into(),
            ));
        }
        if let Some(event) = staged_authority_event(transaction, write)? {
            if event.content.kind.as_str()
                != vela_protocol::authority_history::AUTHORITY_INITIALIZED_EVENT_KIND
            {
                return Err(deny(format!(
                    "fresh repository authority cannot append event kind {}",
                    event.content.kind
                )));
            }
            authority_initializations += 1;
        } else if path == ".vela/origin.json" {
            if write.class != WriteClass::CanonicalEvidence
                || !matches!(write.preimage, FileState::Absent)
                || !matches!(write.postimage, FileState::File { .. })
            {
                return Err(deny(
                    "repository initialization must create one canonical origin object".into(),
                ));
            }
            let bytes = staged_postimage(transaction, write)?;
            repository_origins.push(
                vela_protocol::repository_origin::RepositoryOriginV1::parse(&bytes)
                    .map_err(RepositoryTxnError::CorruptPlan)?,
            );
        } else if path == ".vela/repository.json" {
            if write.class != WriteClass::CanonicalEvidence
                || !matches!(write.preimage, FileState::Absent)
                || !matches!(write.postimage, FileState::File { .. })
            {
                return Err(deny(
                    "repository genesis must create one canonical manifest".into(),
                ));
            }
            let bytes = staged_postimage(transaction, write)?;
            repository_manifests.push(
                vela_protocol::repository::RepositoryV4::parse(&bytes)
                    .map_err(RepositoryTxnError::CorruptPlan)?,
            );
        } else if !path.starts_with(".vela/authority/")
            || write.class != WriteClass::Authority
            || !matches!(write.preimage, FileState::Absent)
            || !matches!(write.postimage, FileState::File { .. })
        {
            return Err(deny(format!(
                "fresh repository authority contains unrelated or non-append write {path}"
            )));
        } else if path.starts_with(".vela/authority/records/") && path.ends_with(".dsse.json") {
            authority_records += 1;
        }
    }

    if authority_initializations != 1 || authority_records != 1 {
        return Err(deny(format!(
            "fresh repository authority requires one initialization event and one covering record; found {authority_initializations} and {authority_records}"
        )));
    }
    if repository_origins.len() != 1 || repository_manifests.len() != 1 {
        return Err(deny(format!(
            "fresh repository authority requires exactly one origin and one repository manifest; found {} origin and {} repository object(s)",
            repository_origins.len(),
            repository_manifests.len()
        )));
    }
    let origin = repository_origins.first().expect("checked one origin");
    let repository = repository_manifests
        .first()
        .expect("checked one repository");
    let origin_root = origin
        .canonical_root()
        .map_err(RepositoryTxnError::CorruptPlan)?;
    if origin.repository_id != expected.repository_id
        || origin.profile_root != expected.profile_root
        || repository.repository_id != expected.repository_id
        || repository.profile_root != expected.profile_root
        || repository.repository_id != origin.repository_id
        || repository.profile_root != origin.profile_root
        || repository.origin_id != origin.id().map_err(deny)?
        || repository.origin_root != origin_root
    {
        return Err(deny(
            "repository origin and manifest do not bind the authorized exact identity".into(),
        ));
    }
    if !repository.accepted_claims.is_empty()
        || !repository.pending_claims.is_empty()
        || !repository.proposals.is_empty()
        || !repository.proposal_withdrawals.is_empty()
        || !repository.submissions.is_empty()
        || !repository.verifications.is_empty()
        || !repository.artifacts.is_empty()
    {
        return Err(deny(
            "fresh repository initialization requires a genesis origin and empty object set".into(),
        ));
    }
    Ok(())
}

/// Acquire and authorize a fresh-repository transaction after checking the
/// bootstrap once before and once after taking the recovery lock.
pub(crate) fn acquire_fresh_repository_write_barrier(
    repository_root: &Path,
    journal_dir: &Path,
) -> Result<CanonicalWriteBarrier, RepositoryTxnError> {
    let root = canonical_repository_path(repository_root)?;
    fresh_context(&root)?;
    let recovery = RepositoryTxn::acquire_recovery_barrier(&root, journal_dir)?;
    let context = fresh_context(recovery.repository_root())?;
    Ok(recovery.authorize(Box::new(RepositoryWriteAuthorization {
        context: PolicyContext::Fresh(context),
        binding: None,
    })))
}

/// Acquire and authorize a routine evidence transaction after checking the
/// current repository and authority heads on both sides of lock acquisition.
pub(crate) fn acquire_routine_evidence_write_barrier(
    repository_root: &Path,
    journal_dir: &Path,
) -> Result<CanonicalWriteBarrier, RepositoryTxnError> {
    let root = canonical_repository_path(repository_root)?;
    routine_context(&root)?;
    let recovery = RepositoryTxn::acquire_recovery_barrier(&root, journal_dir)?;
    let context = routine_context(recovery.repository_root())?;
    Ok(recovery.authorize(Box::new(RepositoryWriteAuthorization {
        context: PolicyContext::Routine(context),
        binding: None,
    })))
}

/// Bind one already-held Decision barrier to the exact verified authority
/// snapshot from planning, and independently confirm that snapshot on disk.
pub(crate) fn authorize_repository_authority_write_barrier(
    recovery: RepositoryRecoveryBarrier,
    repository: &vela_protocol::repository::RepositoryV4,
    authority: &crate::cli::LoadedRepositoryAuthority,
) -> Result<CanonicalWriteBarrier, RepositoryTxnError> {
    let origin = read_repository_origin(recovery.repository_root(), "repository_authority")?;
    let expected = authority_context_from_verified(repository, &origin, authority)?;
    Ok(recovery.authorize(Box::new(RepositoryWriteAuthorization {
        context: PolicyContext::Authority(expected),
        binding: None,
    })))
}

#[cfg(unix)]
fn account_home_from_passwd_buffer(
    directory: *const libc::c_char,
    buffer: &[u8],
) -> Result<PathBuf, RepositoryTxnError> {
    use std::os::unix::ffi::OsStringExt;

    if directory.is_null() {
        return Err(RepositoryTxnError::WriteAuthorization(
            "operating-system account has no home directory".to_string(),
        ));
    }
    let buffer_start = buffer.as_ptr().addr();
    let buffer_end = buffer_start.checked_add(buffer.len()).ok_or_else(|| {
        RepositoryTxnError::WriteAuthorization(
            "operating-system account buffer address overflow".to_string(),
        )
    })?;
    let directory_address = directory.cast::<u8>().addr();
    if directory_address < buffer_start || directory_address >= buffer_end {
        return Err(RepositoryTxnError::WriteAuthorization(
            "operating-system account home directory is outside the password-database buffer"
                .to_string(),
        ));
    }
    let directory = &buffer[directory_address - buffer_start..];
    let length = directory
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| {
            RepositoryTxnError::WriteAuthorization(
                "operating-system account home directory is not NUL-terminated".to_string(),
            )
        })?;
    if length == 0 {
        return Err(RepositoryTxnError::WriteAuthorization(
            "operating-system account has an empty home directory".to_string(),
        ));
    }
    Ok(PathBuf::from(OsString::from_vec(
        directory[..length].to_vec(),
    )))
}

/// Resolve the current operating-system account home without consulting
/// `HOME`, repository configuration, or a process-local override.
#[cfg(unix)]
#[allow(unsafe_code)]
pub(crate) fn operating_system_account_home() -> Result<PathBuf, RepositoryTxnError> {
    // SAFETY: `geteuid` has no preconditions. `getpwuid_r` receives a live
    // passwd allocation, an owned writable buffer, and a result pointer for
    // the duration of each call. A successful call must return that same
    // passwd pointer; the returned `pw_dir` pointer is validated against the buffer
    // before its bytes are copied.
    let uid = unsafe { libc::geteuid() };
    let suggested = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    let mut capacity = if suggested > 0 {
        usize::try_from(suggested).unwrap_or(16 * 1024)
    } else {
        16 * 1024
    }
    .clamp(1024, 1024 * 1024);
    loop {
        let mut buffer = vec![0_u8; capacity];
        let mut passwd = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let passwd_ptr = passwd.as_mut_ptr();
        let mut result = std::ptr::null_mut();
        let status = unsafe {
            libc::getpwuid_r(
                uid,
                passwd_ptr,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE && capacity < 1024 * 1024 {
            capacity = (capacity * 2).min(1024 * 1024);
            continue;
        }
        if status != 0 {
            return Err(RepositoryTxnError::WriteAuthorization(format!(
                "resolve operating-system account home for effective uid {uid}: OS error {status}"
            )));
        }
        if result.is_null() {
            return Err(RepositoryTxnError::WriteAuthorization(format!(
                "operating-system account for effective uid {uid} has no password-database entry"
            )));
        }
        if result != passwd_ptr {
            return Err(RepositoryTxnError::WriteAuthorization(format!(
                "password database returned an unexpected entry pointer for effective uid {uid}"
            )));
        }
        // SAFETY: `getpwuid_r` returned success and identified the exact
        // caller-provided allocation as its initialized result.
        let passwd = unsafe { passwd.assume_init() };
        return account_home_from_passwd_buffer(passwd.pw_dir, &buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    use crate::init::InitOptions;
    use vela_repository::{
        DeltaDraft, OperationId, OperationKind, PlannedWrite, RepoPath, RepositoryBinding,
        RepositoryTxnPlan, RepositoryTxnPlanSpec,
    };

    fn root(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn repository_context() -> RepositoryContext {
        RepositoryContext {
            repository_id: "repository".into(),
            repository_root: root('1'),
            origin_id: "vro_origin".into(),
            origin_root: root('2'),
            final_authority_record_root: root('3'),
            final_authority_event_log_root: root('4'),
            active_keyset_root: root('5'),
            active_authorization_model_root: root('6'),
            authority_event_count: 2,
            authority_record_count: 3,
            closed: false,
            closure_event_id: None,
        }
    }

    fn mutate(value: &mut Value) {
        match value {
            Value::Null => *value = Value::String("present".into()),
            Value::Bool(value) => *value = !*value,
            Value::Number(value) => {
                *value = serde_json::Number::from(value.as_u64().unwrap_or(0) + 1)
            }
            Value::String(value) => value.push_str("-changed"),
            Value::Array(value) => value.push(Value::String("changed".into())),
            Value::Object(value) => {
                value.insert("changed".into(), Value::Bool(true));
            }
        }
    }

    fn leaf_paths(value: &Value, prefix: &mut Vec<String>, paths: &mut Vec<Vec<String>>) {
        if let Value::Object(fields) = value {
            for (name, value) in fields {
                prefix.push(name.clone());
                leaf_paths(value, prefix, paths);
                prefix.pop();
            }
        } else {
            paths.push(prefix.clone());
        }
    }

    fn at_path_mut<'a>(mut value: &'a mut Value, path: &[String]) -> &'a mut Value {
        for component in path {
            value = value.as_object_mut().unwrap().get_mut(component).unwrap();
        }
        value
    }

    fn assert_every_field_is_bound<T: Serialize>(schema: &'static str, context: &T) {
        let plan = ContentDigest::parse(root('c')).unwrap();
        let delta = ContentDigest::parse(root('d')).unwrap();
        let value = serde_json::to_value(context).unwrap();
        let baseline = policy_commitment_root(schema, &value, &plan, &delta).unwrap();
        let verify = |value: &Value, plan: &ContentDigest, delta: &ContentDigest| {
            let actual = policy_commitment_root(schema, value, plan, delta).unwrap();
            (actual == baseline)
                .then_some(())
                .ok_or(RepositoryTxnError::StaleWriteAuthorization {
                    expected: baseline.clone(),
                    actual,
                })
        };
        verify(&value, &plan, &delta).unwrap();
        let mut paths = Vec::new();
        leaf_paths(&value, &mut Vec::new(), &mut paths);
        for path in paths {
            let mut changed = value.clone();
            mutate(at_path_mut(&mut changed, &path));
            assert!(
                matches!(
                    verify(&changed, &plan, &delta),
                    Err(RepositoryTxnError::StaleWriteAuthorization { .. })
                ),
                "context field {} did not fail marker revalidation",
                path.join(".")
            );
        }
        let changed_plan = ContentDigest::parse(root('e')).unwrap();
        assert!(matches!(
            verify(&value, &changed_plan, &delta),
            Err(RepositoryTxnError::StaleWriteAuthorization { .. })
        ));
        let changed_delta = ContentDigest::parse(root('f')).unwrap();
        assert!(matches!(
            verify(&value, &plan, &changed_delta),
            Err(RepositoryTxnError::StaleWriteAuthorization { .. })
        ));
    }

    #[derive(Debug, Clone, Copy)]
    enum FreshDeltaCase {
        ExactGenesis,
        NonemptyGenesis,
        OriginMismatch,
        UnrelatedWrite,
    }

    fn fresh_initialization_write(repository_id: &str) -> PlannedWrite {
        let event = vela_protocol::authority::AuthorityEventV1::new(
            vela_protocol::authority::AuthorityEventContentV1 {
                transaction_id: "vtx_fixture_initialization".into(),
                principal_id: "local:fixture".into(),
                authority_mode: vela_protocol::authority::AUTHORITY_MODE.into(),
                kind: vela_protocol::events::EventKind::Other(
                    vela_protocol::authority_history::AUTHORITY_INITIALIZED_EVENT_KIND.into(),
                ),
                target: vela_protocol::events::StateTarget {
                    r#type: "repository".into(),
                    id: repository_id.into(),
                },
                actor: vela_protocol::events::StateActor {
                    r#type: "human".into(),
                    id: "local:fixture".into(),
                },
                timestamp: "2026-07-29T00:00:00Z".into(),
                reason: "Initialize exact repository authority fixture.".into(),
                before_hash: vela_protocol::events::NULL_HASH.into(),
                after_hash: vela_protocol::events::NULL_HASH.into(),
                payload: json!({}),
                caveats: Vec::new(),
            },
        )
        .unwrap();
        PlannedWrite::write(
            RepoPath::parse(format!(".vela/authority/events/{}.json", event.id)).unwrap(),
            WriteClass::Authority,
            vela_protocol::canonical::to_canonical_bytes(&event).unwrap(),
        )
    }

    fn fresh_bind_result(case: FreshDeltaCase) -> Result<(), RepositoryTxnError> {
        let temporary = tempfile::tempdir().unwrap();
        let repository_root = temporary.path().join("repository");
        crate::init::initialize_minimal(
            &repository_root,
            InitOptions {
                name: "Fresh policy fixture",
                scope: "Exercise the exact fresh repository write policy.",
            },
        )
        .unwrap();
        let profile = crate::repository::verify_bootstrap_at(&repository_root).unwrap();
        let profile_root = profile.profile_root().unwrap();
        let origin = vela_protocol::repository_origin::RepositoryOriginV1::genesis(
            profile.repository_id.clone(),
            profile_root.clone(),
            "Establish current repository authority.".into(),
        )
        .unwrap();
        let mut repository = vela_protocol::repository::RepositoryV4 {
            schema: vela_protocol::repository::REPOSITORY_SCHEMA_V4.into(),
            repository_id: if matches!(case, FreshDeltaCase::OriginMismatch) {
                "ffffffff-ffff-4fff-8fff-ffffffffffff".into()
            } else {
                profile.repository_id.clone()
            },
            profile_root: profile_root.clone(),
            origin_id: origin.id().unwrap(),
            origin_root: origin.canonical_root().unwrap(),
            accepted_claims: Vec::new(),
            pending_claims: Vec::new(),
            proposals: Vec::new(),
            proposal_withdrawals: Vec::new(),
            submissions: Vec::new(),
            verifications: Vec::new(),
            artifacts: Vec::new(),
            authority_keyset_root: root('a'),
            authority_model_root: root('b'),
        };
        if matches!(case, FreshDeltaCase::NonemptyGenesis) {
            repository
                .submissions
                .push(vela_protocol::repository::RepositoryObjectRefV1 {
                    schema: "vela.submission.v1".into(),
                    id: "vsub_fixture".into(),
                    root: root('d'),
                    path: concat!(
                        "records/submissions/sha256/",
                        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd.json"
                    )
                    .into(),
                });
        }

        let mut writes = vec![
            PlannedWrite::write(
                RepoPath::parse(".vela/origin.json").unwrap(),
                WriteClass::CanonicalEvidence,
                origin.canonical_bytes().unwrap(),
            ),
            PlannedWrite::write(
                RepoPath::parse(".vela/repository.json").unwrap(),
                WriteClass::CanonicalEvidence,
                repository.canonical_bytes().unwrap(),
            ),
            fresh_initialization_write(&profile.repository_id),
            // The authority writer has already validated the signed contents;
            // this policy owns only the exact append topology and delta binding.
            PlannedWrite::write(
                RepoPath::parse(".vela/authority/records/var_fixture.dsse.json").unwrap(),
                WriteClass::Authority,
                b"fixture covering record".to_vec(),
            ),
        ];
        if matches!(case, FreshDeltaCase::UnrelatedWrite) {
            writes.push(PlannedWrite::write(
                RepoPath::parse("README.md").unwrap(),
                WriteClass::CanonicalEvidence,
                b"unrelated replacement".to_vec(),
            ));
        }

        let draft = DeltaDraft::prepare(&repository_root, writes).unwrap();
        let plan = RepositoryTxnPlan::new(
            RepositoryTxnPlanSpec {
                kind: OperationKind::new("decision")?,
                operation_id: OperationId::derive(
                    "authority_transaction",
                    format!("fresh-policy-{case:?}").as_bytes(),
                ),
                request_root: ContentDigest::hash(format!("fresh-policy-{case:?}").as_bytes()),
                repository: RepositoryBinding::new(&repository_root, profile.repository_id.clone())
                    .unwrap(),
                fixed_time: "2026-07-29T00:00:00Z".into(),
                read_set: Vec::new(),
                result: json!({"schema": "vela.fresh-policy-fixture.v1"}),
            },
            draft.delta.clone(),
        )
        .unwrap();
        let journal_dir = repository_root.join(".vela/operation-journals");
        let barrier = acquire_fresh_repository_write_barrier(&repository_root, &journal_dir)?;
        RepositoryTxn::prepare_with_barrier(barrier, plan, draft).map(|_| ())
    }

    #[test]
    fn fresh_policy_bind_accepts_exact_empty_genesis_delta() {
        fresh_bind_result(FreshDeltaCase::ExactGenesis).unwrap();
    }

    #[test]
    fn fresh_policy_bind_rejects_nonempty_mismatched_and_unrelated_deltas() {
        for (case, reason) in [
            (FreshDeltaCase::NonemptyGenesis, "empty object set"),
            (FreshDeltaCase::OriginMismatch, "exact identity"),
            (
                FreshDeltaCase::UnrelatedWrite,
                "unrelated or non-append write",
            ),
        ] {
            let error = fresh_bind_result(case).unwrap_err();
            match error {
                RepositoryTxnError::RepositoryWriteIntentDenied {
                    intent: "repository_authority_initialization",
                    reason: actual,
                } => assert!(
                    actual.contains(reason),
                    "{case:?} returned unexpected denial: {actual}"
                ),
                error => panic!("{case:?} returned unexpected error: {error}"),
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn policy_context_rejects_lossy_non_utf8_paths() {
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(vec![b'/', b'h', 0xff]));
        assert!(matches!(
            exact_path_string(&path, "test path"),
            Err(RepositoryTxnError::WriteAuthorization(error))
                if error.contains("not valid UTF-8")
        ));
    }

    #[test]
    fn fresh_context_commits_every_field_plan_and_delta() {
        assert_every_field_is_bound(
            FRESH_CONTEXT_SCHEMA,
            &FreshContext {
                repository_id: "repository".into(),
                profile_root: root('1'),
                canonical_os_home: "/home/operator".into(),
                bootstrap_absence_root: root('2'),
                trust_anchor_path: "/home/operator/.vela/trust/repository.json".into(),
                trust_anchor_absent: true,
            },
        );
    }

    #[test]
    fn routine_context_commits_every_field_plan_and_delta() {
        assert_every_field_is_bound(ROUTINE_CONTEXT_SCHEMA, &repository_context());
    }

    #[test]
    fn authority_context_commits_every_field_plan_and_delta() {
        assert_every_field_is_bound(
            AUTHORITY_CONTEXT_SCHEMA,
            &AuthorityContext {
                repository: repository_context(),
                canonical_os_home: "/home/operator".into(),
                trust_anchor_path: "/home/operator/.vela/trust/repository.json".into(),
                trust_anchor_root: root('7'),
                sequence_one_record_root: root('8'),
                initialization_event_id: "vev_initialization".into(),
                initialization_event_root: root('9'),
            },
        );
    }

    #[cfg(unix)]
    #[test]
    fn account_home_copy_retains_exact_in_buffer_bytes() {
        use std::os::unix::ffi::OsStrExt;

        let buffer = b"prefix/tmp/\xff\0ignored".to_vec();
        let directory = buffer
            .as_ptr()
            .wrapping_add(b"prefix".len())
            .cast::<libc::c_char>();
        let home = account_home_from_passwd_buffer(directory, &buffer).unwrap();
        assert_eq!(home.as_os_str().as_bytes(), b"/tmp/\xff");
    }

    #[cfg(unix)]
    #[test]
    fn account_home_copy_rejects_null_and_out_of_buffer_pointers() {
        let buffer = b"/home/operator\0".to_vec();
        assert!(matches!(
            account_home_from_passwd_buffer(std::ptr::null(), &buffer),
            Err(RepositoryTxnError::WriteAuthorization(error))
                if error == "operating-system account has no home directory"
        ));

        let outside = b"/outside\0";
        assert!(matches!(
            account_home_from_passwd_buffer(outside.as_ptr().cast(), &buffer),
            Err(RepositoryTxnError::WriteAuthorization(error))
                if error.contains("outside the password-database buffer")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn account_home_copy_rejects_unterminated_and_empty_directories() {
        let unterminated = b"/home/operator".to_vec();
        assert!(matches!(
            account_home_from_passwd_buffer(unterminated.as_ptr().cast(), &unterminated),
            Err(RepositoryTxnError::WriteAuthorization(error))
                if error.contains("not NUL-terminated")
        ));

        let empty = vec![0_u8];
        assert!(matches!(
            account_home_from_passwd_buffer(empty.as_ptr().cast(), &empty),
            Err(RepositoryTxnError::WriteAuthorization(error))
                if error == "operating-system account has an empty home directory"
        ));
    }

    #[test]
    fn operating_system_account_home_ignores_hostile_home_environment() {
        const CHILD: &str = "VELA_OS_ACCOUNT_HOME_REDIRECTION_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let hostile = fs::canonicalize(
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .expect("child HOME is set"),
            )
            .unwrap();
            let actual = fs::canonicalize(operating_system_account_home().unwrap()).unwrap();
            assert_ne!(actual, hostile, "hostile HOME redirected trust-pin lookup");
            return;
        }

        let attacker_home = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(
                "repository_write_policy::tests::operating_system_account_home_ignores_hostile_home_environment",
            )
            .arg("--nocapture")
            .env(CHILD, "1")
            .env("HOME", attacker_home.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "hostile-HOME child failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
