//! Explicit operator recovery for one exact durable repository transaction.
//!
//! The repository runtime owns every state transition. This module only binds
//! the CLI request, classifies product errors, and renders the stable result.

use std::path::Path;

use serde::Serialize;
use vela_repository::{
    OperationId, RecoveryOutcome, RepositoryRecoveryResult, RepositoryTxn, RepositoryTxnError,
};

use crate::ui::{self, ErrorKind};

#[derive(Serialize)]
struct RecoverResultV1<'a> {
    schema: &'static str,
    ok: bool,
    command: &'static str,
    repository_path: &'a str,
    repository_id: &'a str,
    operation_id: &'a str,
    prior_recovery_state: &'static str,
    outcome: &'static str,
    repository_blocked_after: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_command: Option<String>,
}

pub(super) fn cmd_recover(repository: &Path, operation_id: &str, json: bool) {
    ui::set_mode("recover", json);
    let operation_id = OperationId::parse(operation_id.to_string()).unwrap_or_else(|error| {
        ui::fail_with(
            ErrorKind::Usage,
            &error.to_string(),
            Some("pass the exact vop_ operation id printed by the blocked write"),
        )
    });
    let repository = ui::canonicalize_repo(repository);
    let private = repository.join(".vela");
    match std::fs::symlink_metadata(&private) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => ui::fail_with(
            ErrorKind::NotFound,
            &format!(
                "repository private directory does not exist: {}",
                private.display()
            ),
            Some("pass the exact repository path named by the blocked write"),
        ),
        Err(error) => ui::fail_coded(
            ErrorKind::Domain,
            Some("repository_incomplete"),
            &format!(
                "inspect repository private directory {}: {error}",
                private.display()
            ),
            None,
        ),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => ui::fail_coded(
            ErrorKind::Domain,
            Some("repository_incomplete"),
            &format!(
                "repository private directory must be a real directory: {}",
                private.display()
            ),
            None,
        ),
        Ok(_) => {}
    }
    let journal_dir = crate::repository_ops::repository_transaction_journal_dir(&repository)
        .unwrap_or_else(|error| {
            ui::fail_coded(
                ErrorKind::Domain,
                Some("repository_incomplete"),
                &error,
                None,
            )
        });
    let profile = crate::repository::verify_profile_at(&repository).unwrap_or_else(|error| {
        ui::fail_coded(
            ErrorKind::Domain,
            Some("repository_incomplete"),
            &error,
            Some(
                "restore the exact current vela.toml from trusted repository history; never derive repository identity from a recovery journal",
            ),
        )
    });
    #[cfg(feature = "test-support")]
    let recovery = if std::env::var_os("VELA_TEST_INTERRUPT_RECOVERY_AFTER_INSTALLED").is_some() {
        RepositoryTxn::recover_at_failpoint(
            &repository,
            &journal_dir,
            &operation_id,
            &profile.repository_id,
            vela_repository::RepositoryTxnStep::AfterInstalledJournalWrite,
        )
    } else {
        RepositoryTxn::recover(
            &repository,
            &journal_dir,
            &operation_id,
            &profile.repository_id,
        )
    };
    #[cfg(not(feature = "test-support"))]
    let recovery = RepositoryTxn::recover(
        &repository,
        &journal_dir,
        &operation_id,
        &profile.repository_id,
    );
    let result = recovery.unwrap_or_else(|error| fail_recovery(&repository, &operation_id, error));
    render_recovery(&repository, &result, json);
}

fn fail_recovery(repository: &Path, operation_id: &OperationId, error: RepositoryTxnError) -> ! {
    if matches!(error, RepositoryTxnError::OperationNotFound { .. }) {
        ui::fail_with(
            ErrorKind::NotFound,
            &error.to_string(),
            Some("use the exact operation id printed by the blocked write"),
        );
    }
    let retry = format!(
        "vela recover --repo {} {} --json",
        super::shell_arg(&repository.display().to_string()),
        operation_id.as_str()
    );
    if matches!(error, RepositoryTxnError::Busy) {
        ui::fail_with(
            ErrorKind::Domain,
            &error.to_string(),
            Some(&format!(
                "wait for the active repository writer to exit, then rerun: {retry}"
            )),
        );
    }
    let hint = match &error {
        RepositoryTxnError::CommittedConflict { path, .. } => format!(
            "inspect and restore the exact committed preimage or postimage at {}; then rerun: {retry}",
            path.as_str()
        ),
        RepositoryTxnError::CompletedPostimageMismatch { path, .. } => format!(
            "restore transaction-consistent repository bytes at {} from a trusted repository backup; do not alter the valid journal evidence; then rerun: {retry}",
            path.as_str()
        ),
        RepositoryTxnError::UnsafeTarget { path, .. } => format!(
            "remove the path substitution and restore a real repository target at {}; then rerun: {retry}",
            path.as_str()
        ),
        RepositoryTxnError::CorruptPlan(message)
            if message.contains("impossible durable recovery layout") =>
        {
            format!(
                "preserve the durable marker, inspect the transaction plan paths and progress, and restore transaction-consistent repository bytes from trusted evidence; then rerun: {retry}"
            )
        }
        RepositoryTxnError::MissingBlob(_) | RepositoryTxnError::CorruptBlob(_) => format!(
            "restore the exact private recovery blob from a trusted backup; never synthesize replacement bytes; then rerun: {retry}"
        ),
        RepositoryTxnError::RepositoryBindingMismatch { expected, .. } => format!(
            "return the repository to its original canonical path, then rerun: vela recover --repo {} {} --json",
            super::shell_arg(expected),
            operation_id.as_str()
        ),
        RepositoryTxnError::RepositoryIdentityMismatch { expected, actual } => format!(
            "this repository profile identifies {expected}, but the journal belongs to {actual}; recover it only in the exact matching repository and never rewrite either identity"
        ),
        RepositoryTxnError::AmbiguousRecovery { .. }
        | RepositoryTxnError::MultiplePendingTransactions { .. } =>
            "restore the private journal set to one valid incomplete operation; never choose or delete a journal by directory order"
                .to_string(),
        _ => format!(
            "restore the exact private journal and marker bytes from a trusted backup; never delete or reinterpret a marker as absent; then rerun: {retry}"
        ),
    };
    ui::fail_coded(
        ErrorKind::Domain,
        Some("repository_incomplete"),
        &error.to_string(),
        Some(&hint),
    )
}

fn render_recovery(repository: &Path, result: &RepositoryRecoveryResult, json: bool) {
    let repository_path = repository.to_str().unwrap_or_else(|| {
        ui::fail_with(
            ErrorKind::Domain,
            "repository path is not valid UTF-8",
            None,
        )
    });
    let outcome = outcome_token(result.outcome);
    let next_command = if let Some(operation_id) = &result.next_operation_id {
        Some(format!(
            "vela recover --repo {} {} --json",
            super::shell_arg(repository_path),
            operation_id.as_str()
        ))
    } else if matches!(
        result.outcome,
        RecoveryOutcome::Completed | RecoveryOutcome::AlreadyCompleted
    ) {
        match super::completed_native_genesis_init_command(repository, &result.operation_id) {
            Ok(Some(command)) => Some(command),
            Ok(None) => Some(format!(
                "git -C {} status --short",
                super::shell_arg(repository_path)
            )),
            Err(error) => ui::fail_coded(
                ErrorKind::Domain,
                Some("repository_incomplete"),
                &format!(
                    "recovery completed the exact filesystem transaction, but its native-genesis continuation could not be verified: {error}"
                ),
                Some(
                    "preserve the Completed journal and exact repository bytes; repair the native-genesis proof before rerunning the exact `vela init`; recovery never publishes Git or installs trust",
                ),
            ),
        }
    } else {
        None
    };
    let payload = RecoverResultV1 {
        schema: "vela.recover-result.v1",
        ok: true,
        command: "recover",
        repository_path,
        repository_id: &result.repository_id,
        operation_id: result.operation_id.as_str(),
        prior_recovery_state: result.prior_state.as_str(),
        outcome,
        repository_blocked_after: result.next_operation_id.is_some(),
        next_command,
    };
    if json {
        super::print_json(&payload);
        return;
    }

    ui::header("RECOVERY", result.operation_id.as_str(), Some(outcome));
    println!(
        "  {:<24} {}",
        crate::style::dim("repository"),
        super::safe_text::inline(repository_path)
    );
    println!(
        "  {:<24} {}",
        crate::style::dim("repository id"),
        super::safe_text::inline(&result.repository_id)
    );
    println!(
        "  {:<24} {}",
        crate::style::dim("prior state"),
        result.prior_state.as_str()
    );
    println!(
        "  {:<24} {}",
        crate::style::dim("repository blocked"),
        if result.next_operation_id.is_some() {
            "yes"
        } else {
            "no"
        }
    );
    println!(
        "  {:<24} not attempted",
        crate::style::dim("Git publication")
    );
    if let Some(next) = payload.next_command {
        println!(
            "  {:<24} {}",
            crate::style::dim("next"),
            super::safe_text::inline(&next)
        );
    }
}

fn outcome_token(outcome: RecoveryOutcome) -> &'static str {
    match outcome {
        RecoveryOutcome::Completed => "completed",
        RecoveryOutcome::AbortedPrepared => "aborted_prepared",
        RecoveryOutcome::AlreadyCompleted => "already_completed",
        RecoveryOutcome::AlreadyAborted => "already_aborted",
    }
}
