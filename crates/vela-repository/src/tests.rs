use super::*;
use serde_json::json;

const TEST_REPOSITORY_ID: &str = "33333333-3333-4333-8333-333333333333";
const TEST_OWNED_ATOMIC_TEMP: &str = ".vela-journal-tmp-abcdefghijkl";

fn empty_test_repository() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repository");
    let journals = temp.path().join("journals");
    fs::create_dir_all(&root).unwrap();
    (temp, root, journals)
}

fn fixture_root(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn fixture_plan(root: &Path, draft: &DeltaDraft, identity: &[u8]) -> RepositoryTxnPlan {
    let operation_id = OperationId::derive("submission", identity);
    let request_root = ContentDigest::hash(identity);
    RepositoryTxnPlan::new(
        RepositoryTxnPlanSpec {
            kind: OperationKind::new("submission").unwrap(),
            operation_id,
            request_root,
            repository: RepositoryBinding::new(root, TEST_REPOSITORY_ID).unwrap(),
            fixed_time: "2026-07-13T00:00:00Z".to_string(),
            read_set: vec![InputBinding {
                name: "receipt".to_string(),
                digest: ContentDigest::hash(b"receipt"),
            }],
            result: json!({"proposal_id": "vpr_test"}),
        },
        draft.delta.clone(),
    )
    .unwrap()
}

fn one_write_fixture(root: &Path, path: &str, identity: &[u8]) -> (DeltaDraft, RepositoryTxnPlan) {
    let draft = DeltaDraft::prepare(
        root,
        vec![PlannedWrite::write(
            RepoPath::parse(path).unwrap(),
            WriteClass::CanonicalEvidence,
            b"authorized postimage".to_vec(),
        )],
    )
    .unwrap();
    let plan = fixture_plan(root, &draft, identity);
    (draft, plan)
}

fn persist_marker_free_prepared_fixture(
    root: &Path,
    journals: &Path,
    path: &str,
    identity: &[u8],
) -> (OperationId, RepositoryTxnPaths) {
    let (draft, plan) = one_write_fixture(root, path, identity);
    let operation_id = plan.operation_id.clone();
    let txn = RepositoryTxn::prepare(root, journals, plan, draft).unwrap();
    let paths = txn.paths.clone();
    drop(txn);
    (operation_id, paths)
}

fn mark_install_complete(mut transaction: RepositoryTxn) -> RepositoryTxnPaths {
    transaction.mark_committed().unwrap();
    transaction.install().unwrap();
    transaction.complete().unwrap();
    transaction.paths.clone()
}

#[test]
fn operation_kind_is_validated_at_construction_and_durable_plan_verification() {
    for value in [
        "submission",
        "proposal_withdrawal",
        "verification",
        "decision",
    ] {
        OperationKind::new(value).unwrap();
    }
    for value in [
        "",
        "Decision With Spaces",
        "_leading",
        "trailing_",
        "double__separator",
        "contains-hyphen",
    ] {
        assert!(matches!(
            OperationKind::new(value),
            Err(RepositoryTxnError::CorruptPlan(error))
                if error.contains("invalid internal operation kind")
        ));
    }
    assert!(matches!(
        OperationKind::new("a".repeat(65)),
        Err(RepositoryTxnError::CorruptPlan(error))
            if error.contains("invalid internal operation kind")
    ));

    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repository");
    fs::create_dir_all(&root).unwrap();
    assert!(matches!(
        RepositoryBinding::new(&root, "   "),
        Err(RepositoryTxnError::CorruptPlan(error))
            if error.contains("empty repository id")
    ));
    assert!(matches!(
        RepositoryBinding::new(&root, "r".repeat(MAX_REPOSITORY_ID_BYTES + 1)),
        Err(RepositoryTxnError::CorruptPlan(error))
            if error.contains("exceeds 256 bytes")
    ));
    let (draft, mut plan) = one_write_fixture(&root, "records/operation.json", b"operation kind");
    assert_eq!(draft.delta, plan.canonical_delta);
    plan.kind = OperationKind("invalid kind".into());
    plan.root = plan.compute_root().unwrap();
    assert!(matches!(
        plan.verify(),
        Err(RepositoryTxnError::CorruptPlan(error))
            if error.contains("invalid internal operation kind")
    ));
}

#[test]
fn recovery_state_tokens_are_stable_and_progress_independent() {
    assert_eq!(RecoveryState::Prepared.as_str(), "prepared");
    assert_eq!(RecoveryState::Aborted.as_str(), "aborted");
    assert_eq!(RecoveryState::Committed.as_str(), "committed");
    assert_eq!(
        RecoveryState::Installing {
            installed: 3,
            total: 7,
        }
        .as_str(),
        "installing"
    );
    assert_eq!(RecoveryState::Installed.as_str(), "installed");
    assert_eq!(RecoveryState::Completed.as_str(), "completed");
    assert_eq!(
        RecoveryState::CommittedConflict {
            path: RepoPath::parse("records/conflict.json").unwrap(),
        }
        .as_str(),
        "committed_conflict"
    );
}

#[cfg(unix)]
#[test]
fn repository_root_or_filesystem_rejects_non_utf8_paths_instead_of_lossy_binding() {
    use std::os::unix::ffi::OsStringExt;

    let temporary = tempfile::tempdir().unwrap();
    let root = temporary
        .path()
        .join(std::ffi::OsString::from_vec(vec![b'r', 0xff]));
    if fs::create_dir(&root).is_err() {
        // Some supported filesystems reject the byte sequence before the
        // runtime can canonicalize it. That is already fail-closed.
        return;
    }
    assert!(matches!(
        canonical_repository_root(&root),
        Err(RepositoryTxnError::Io(error)) if error.contains("not valid UTF-8")
    ));
}

#[test]
fn authorization_bind_denial_precedes_every_transaction_journal_byte() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repository");
    let journals = temporary.path().join("journals");
    fs::create_dir_all(&root).unwrap();
    let (draft, plan) = one_write_fixture(&root, "records/denied.json", b"deny bind");
    let paths = RepositoryTxnPaths::new(&journals, &plan.operation_id);
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let barrier = RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap();
    let error = RepositoryTxn::prepare_with_recovery_barrier_and_authorization(
        barrier,
        Box::new(RecordingTransactionAuthorization {
            deny_bind: true,
            calls: Some(calls.clone()),
            ..Default::default()
        }),
        plan,
        draft,
        &mut NoRepositoryTxnFailpoints,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "test_transaction",
            ..
        }
    ));
    assert_eq!(*calls.lock().unwrap(), ["bind_plan"]);
    assert!(!paths.plan.exists());
    assert!(!paths.marker.exists());
    assert!(!paths.blob_dir.exists());
    assert!(!paths.plan.parent().expect("journal parent").exists());
    assert!(!root.join("records/denied.json").exists());
}

#[test]
fn authorization_refuses_same_delta_transplanted_across_any_plan_metadata() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repository");
    let journals = temporary.path().join("journals");
    fs::create_dir_all(&root).unwrap();
    let (draft, authorized) =
        one_write_fixture(&root, "records/exact-plan.json", b"authorized plan");
    let binding = TestAuthorizationBinding {
        repository_id: authorized.repository.repository_id.clone(),
        plan_root: authorized.root.clone(),
        delta_root: authorized.canonical_delta.root().clone(),
    };
    let mut variants = Vec::new();
    let mut mutate = |name: &'static str, update: fn(&mut RepositoryTxnPlan)| {
        let mut plan = authorized.clone();
        update(&mut plan);
        plan.root = plan.compute_root().unwrap();
        assert_eq!(authorized.canonical_delta, plan.canonical_delta);
        assert_ne!(
            authorized.root, plan.root,
            "{name} must change the plan root"
        );
        variants.push((name, plan));
    };
    mutate("repository id binding", |plan| {
        plan.repository.repository_id = "44444444-4444-4444-8444-444444444444".into()
    });
    mutate("repository root binding", |plan| {
        plan.repository.canonical_root = "/different/repository".into()
    });
    mutate("operation kind", |plan| {
        plan.kind = OperationKind::new("verification").unwrap()
    });
    mutate("operation id", |plan| {
        plan.operation_id = OperationId::derive("submission", b"different operation")
    });
    mutate("request", |plan| {
        plan.request_root = ContentDigest::hash(b"different request")
    });
    mutate("time", |plan| {
        plan.fixed_time = "2026-07-14T00:00:00Z".into()
    });
    mutate("read set", |plan| {
        plan.read_set.push(InputBinding {
            name: "another input".into(),
            digest: ContentDigest::hash(b"another input"),
        })
    });
    mutate("result", |plan| {
        plan.result = json!({"proposal_id": "vpr_different"})
    });

    for (name, plan) in variants {
        let paths = RepositoryTxnPaths::new(&journals, &plan.operation_id);
        let barrier = RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap();
        let error = RepositoryTxn::prepare_with_recovery_barrier_and_authorization(
            barrier,
            Box::new(RecordingTransactionAuthorization {
                binding: Some(binding.clone()),
                ..Default::default()
            }),
            plan,
            draft.clone(),
            &mut NoRepositoryTxnFailpoints,
        )
        .unwrap_err();

        assert!(
            matches!(
                error,
                RepositoryTxnError::StaleWriteAuthorization { .. }
                    | RepositoryTxnError::WriteAuthorizationRepositoryMismatch { .. }
                    | RepositoryTxnError::RepositoryBindingMismatch { .. }
            ),
            "{name} transplant returned {error}"
        );
        assert!(!paths.plan.exists(), "{name} wrote a durable plan");
        assert!(!paths.marker.exists(), "{name} wrote a marker");
    }
    assert!(!root.join("records/exact-plan.json").exists());
}

#[test]
fn authorization_marker_denial_aborts_without_repository_delta() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repository");
    let journals = temporary.path().join("journals");
    fs::create_dir_all(&root).unwrap();
    let (draft, plan) = one_write_fixture(&root, "records/denied.json", b"deny marker");
    let paths = RepositoryTxnPaths::new(&journals, &plan.operation_id);
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let barrier = RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap();
    let mut transaction = RepositoryTxn::prepare_with_recovery_barrier_and_authorization(
        barrier,
        Box::new(RecordingTransactionAuthorization {
            deny_marker: true,
            calls: Some(calls.clone()),
            ..Default::default()
        }),
        plan,
        draft,
        &mut NoRepositoryTxnFailpoints,
    )
    .unwrap();

    let error = transaction.mark_committed().unwrap_err();
    assert!(matches!(
        error,
        RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "test_transaction",
            ..
        }
    ));
    assert_eq!(transaction.recovery_state(), &RecoveryState::Aborted);
    assert!(transaction.authorization.is_none());
    assert_eq!(
        *calls.lock().unwrap(),
        ["bind_plan", "revalidate_for_marker"]
    );
    assert!(!paths.marker.exists());
    assert!(!root.join("records/denied.json").exists());
}

#[test]
fn generic_preflight_failure_precedes_marker_revalidation() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repository");
    let journals = temporary.path().join("journals");
    fs::create_dir_all(&root).unwrap();
    let (draft, plan) = one_write_fixture(&root, "records/drift.json", b"generic first");
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let barrier = RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap();
    let mut transaction = RepositoryTxn::prepare_with_recovery_barrier_and_authorization(
        barrier,
        Box::new(RecordingTransactionAuthorization {
            calls: Some(calls.clone()),
            ..Default::default()
        }),
        plan,
        draft,
        &mut NoRepositoryTxnFailpoints,
    )
    .unwrap();
    fs::create_dir_all(root.join("records")).unwrap();
    fs::write(root.join("records/drift.json"), b"ambient drift").unwrap();

    assert!(matches!(
        transaction.mark_committed(),
        Err(RepositoryTxnError::StalePreimage { .. })
    ));
    assert_eq!(*calls.lock().unwrap(), ["bind_plan"]);
    assert_eq!(transaction.recovery_state(), &RecoveryState::Aborted);
    assert!(transaction.authorization.is_none());
    assert!(!transaction.paths.marker.exists());
}

#[test]
fn malformed_marker_cannot_preserve_authorization_through_any_lifecycle_reader() {
    for action in ["commit", "abort", "install"] {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("repository");
        let journals = temporary.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let relative = format!("records/malformed-marker-{action}.json");
        let (draft, plan) = one_write_fixture(&root, &relative, action.as_bytes());
        let barrier = RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap();
        let mut transaction = RepositoryTxn::prepare_with_recovery_barrier_and_authorization(
            barrier,
            Box::new(RecordingTransactionAuthorization::default()),
            plan,
            draft,
            &mut NoRepositoryTxnFailpoints,
        )
        .unwrap();
        fs::create_dir_all(transaction.paths.marker.parent().unwrap()).unwrap();
        fs::write(&transaction.paths.marker, b"{").unwrap();

        let error = match action {
            "commit" => transaction.mark_committed(),
            "abort" => transaction.abort_prepared(),
            "install" => transaction.install(),
            _ => unreachable!(),
        }
        .unwrap_err();
        assert!(matches!(error, RepositoryTxnError::Journal(_)));
        assert!(transaction.authorization.is_none(), "{action}");
        assert_eq!(transaction.recovery_state(), &RecoveryState::Prepared);
        fs::remove_file(&transaction.paths.marker).unwrap();
        assert!(matches!(
            transaction.mark_committed(),
            Err(RepositoryTxnError::WriteAuthorizationRequired { .. })
        ));
        assert!(!root.join(relative).exists());
    }
}

#[test]
fn marker_write_error_cannot_retain_in_memory_authorization() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repository");
    let journals = temporary.path().join("journals");
    fs::create_dir_all(&root).unwrap();
    let (draft, plan) = one_write_fixture(
        &root,
        "records/marker-write-error.json",
        b"marker write error",
    );
    let paths = RepositoryTxnPaths::new(&journals, &plan.operation_id);
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let barrier = RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap();
    let mut transaction = RepositoryTxn::prepare_with_recovery_barrier_and_authorization(
        barrier,
        Box::new(RecordingTransactionAuthorization {
            obstruct_marker_write_on_revalidate: Some(paths.marker.clone()),
            calls: Some(calls.clone()),
            ..Default::default()
        }),
        plan,
        draft,
        &mut NoRepositoryTxnFailpoints,
    )
    .unwrap();

    assert!(matches!(
        transaction.mark_committed(),
        Err(RepositoryTxnError::Journal(_))
    ));
    assert_eq!(
        *calls.lock().unwrap(),
        ["bind_plan", "revalidate_for_marker"]
    );
    assert!(transaction.authorization.is_none());
    assert_eq!(transaction.recovery_state(), &RecoveryState::Prepared);
    assert!(paths.marker.is_dir());
    assert!(!root.join("records/marker-write-error.json").exists());
}

#[test]
fn marker_present_recovery_never_invokes_authorization() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repository");
    let journals = temporary.path().join("journals");
    fs::create_dir_all(&root).unwrap();
    let (draft, plan) = one_write_fixture(&root, "records/recover.json", b"marker recovery");
    let operation_id = plan.operation_id.clone();
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let barrier = RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap();
    let mut transaction = RepositoryTxn::prepare_with_recovery_barrier_and_authorization(
        barrier,
        Box::new(RecordingTransactionAuthorization {
            calls: Some(calls.clone()),
            ..Default::default()
        }),
        plan,
        draft,
        &mut NoRepositoryTxnFailpoints,
    )
    .unwrap();
    assert_injected(
        transaction.mark_committed_at_failpoint(RepositoryTxnStep::AfterCommitMarkerWrite),
        RepositoryTxnStep::AfterCommitMarkerWrite,
    );
    assert!(transaction.authorization.is_none());
    calls.lock().unwrap().clear();
    drop(transaction);

    let recovery =
        RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID).unwrap();
    assert_eq!(recovery.operation_id, operation_id);
    assert_eq!(recovery.prior_state, RecoveryState::Prepared);
    assert_eq!(recovery.outcome, RecoveryOutcome::Completed);
    assert!(recovery.next_operation_id.is_none());
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(
        fs::read(root.join("records/recover.json")).unwrap(),
        b"authorized postimage"
    );
}

#[test]
fn reopened_prepared_journal_requires_fresh_exact_test_binding() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repository");
    let journals = temporary.path().join("journals");
    fs::create_dir_all(&root).unwrap();
    let (draft, plan) = one_write_fixture(&root, "records/reopen.json", b"reopen binding");
    let operation_id = plan.operation_id.clone();
    drop(RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap());

    let mut reopened = RepositoryTxn::open(&root, &journals, &operation_id).unwrap();
    assert!(matches!(
        reopened.mark_committed(),
        Err(RepositoryTxnError::WriteAuthorizationRequired { .. })
    ));
    assert_eq!(reopened.recovery_state(), &RecoveryState::Prepared);
    assert!(!reopened.paths.marker.exists());
    reopened.bind_exact_test_authorization().unwrap();
    reopened.mark_committed().unwrap();
    reopened.install().unwrap();
    reopened.complete().unwrap();
    assert_eq!(
        fs::read(root.join("records/reopen.json")).unwrap(),
        b"authorized postimage"
    );
}

#[test]
fn authorization_postimage_reads_are_bounded_to_exact_delta_members() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (draft, plan) = one_write_fixture(root, "records/member.json", b"bounded read");
    let mut foreign = draft.delta.writes()[0].clone();
    foreign.path = RepoPath::parse("records/foreign.json").unwrap();
    let mut read_attempted = false;
    let mut read_blob = |_blob: &JournalBlobRef| {
        read_attempted = true;
        Ok(Vec::new())
    };
    let mut context = TransactionAuthorizationContext::new(
        root,
        &plan.repository,
        &plan.root,
        &draft.delta,
        &mut read_blob,
    );

    assert!(matches!(
        context.postimage_bytes(&foreign),
        Err(RepositoryTxnError::CorruptPlan(_))
    ));
    assert!(!read_attempted);
}

#[test]
fn post_phase_one_transaction_wire_fixture_is_byte_exact() {
    let temporary = tempfile::tempdir().unwrap();
    let repository = temporary.path().join("repository");
    fs::create_dir_all(repository.join(".vela")).unwrap();
    fs::write(
        repository.join(".vela/repository.json"),
        b"{\"schema\":\"fixture.before\"}\n",
    )
    .unwrap();

    let draft = DeltaDraft::prepare(
        &repository,
        vec![
            PlannedWrite::write(
                RepoPath::parse(".vela/repository.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"{\"schema\":\"fixture.after\"}\n".to_vec(),
            ),
            PlannedWrite::write(
                RepoPath::parse(concat!(
                    "records/proposals/sha256/",
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.json"
                ))
                .unwrap(),
                WriteClass::PublicReview,
                b"{\"schema\":\"vela.proposal.v1\",\"id\":\"vpr_fixture\"}\n".to_vec(),
            ),
        ],
    )
    .unwrap();

    // RepositoryBinding normally retains the canonical absolute checkout
    // path. That fact intentionally varies between machines and belongs in
    // the live binding check, not a wire golden. This sentinel exercises
    // the exact current serialization without goldening a temporary path.
    let plan = RepositoryTxnPlan::new(
        RepositoryTxnPlanSpec {
            kind: OperationKind::new("submission").unwrap(),
            operation_id: OperationId::derive(
                "submission",
                b"post-phase-one-transaction-wire-fixture",
            ),
            request_root: ContentDigest::hash(b"post-phase-one-transaction-wire-fixture-request"),
            repository: RepositoryBinding {
                canonical_root: "/__vela_repository_txn_wire_fixture__/repository".into(),
                repository_id: "33333333-3333-4333-8333-333333333333".into(),
            },
            fixed_time: "2026-08-10T00:00:00Z".into(),
            read_set: vec![
                InputBinding {
                    name: "current_repository_before".into(),
                    digest: ContentDigest::hash(b"fixture repository input"),
                },
                InputBinding {
                    name: "submission".into(),
                    digest: ContentDigest::hash(b"fixture submission input"),
                },
            ],
            result: json!({
                "accepted_event_delta": 0,
                "proposal_id": "vpr_fixture",
                "schema": "vela.routine-evidence-transaction-result.internal.v1"
            }),
        },
        draft.delta.clone(),
    )
    .unwrap();
    let marker = CommitMarker::from_plan(&plan);
    let journal = RepositoryTxnJournal {
        schema: REPOSITORY_TXN_SCHEMA.into(),
        plan: plan.clone(),
        recovery: RecoveryState::Prepared,
        blob_retention: BlobRetention::Retained,
    };
    draft.delta.verify().unwrap();
    plan.verify().unwrap();
    journal.verify().unwrap();

    let delta_bytes = vela_protocol::canonical::to_canonical_bytes(&draft.delta).unwrap();
    let decoded_delta: CanonicalDelta = serde_json::from_slice(&delta_bytes).unwrap();
    assert_eq!(decoded_delta, draft.delta);
    assert_eq!(
        vela_protocol::canonical::to_canonical_bytes(&decoded_delta).unwrap(),
        delta_bytes
    );
    assert_eq!(
        draft.delta.root().as_str(),
        "sha256:f08a454ab496dbea12f8e3b2e2ab2a69a08ef06b0f8f502f517922ebe418e276"
    );
    assert_eq!(delta_bytes.len(), 981);
    assert_eq!(
        ContentDigest::hash(&delta_bytes).as_str(),
        "sha256:2abe20d66a345c8516d26663b7022c052e02e9f5041c1c1a6e1363412710a8e7"
    );

    let plan_bytes = vela_protocol::canonical::to_canonical_bytes(&plan).unwrap();
    let decoded_plan: RepositoryTxnPlan = serde_json::from_slice(&plan_bytes).unwrap();
    assert_eq!(decoded_plan, plan);
    assert_eq!(
        vela_protocol::canonical::to_canonical_bytes(&decoded_plan).unwrap(),
        plan_bytes
    );
    assert_eq!(
        plan.root().as_str(),
        "sha256:e5a1739a0912088d305bdae68a02f04163d649c7b2dd5ddbe801a4f91c4649f4"
    );
    assert_eq!(plan_bytes.len(), 1860);
    assert_eq!(
        ContentDigest::hash(&plan_bytes).as_str(),
        "sha256:8e42a1618f3a4cc3e6ca077def97574d7c6aa8f06bfdfe7872269d112f5a08e6"
    );

    let marker_bytes = vela_protocol::canonical::to_canonical_bytes(&marker).unwrap();
    let decoded_marker: CommitMarker = serde_json::from_slice(&marker_bytes).unwrap();
    assert_eq!(decoded_marker, marker);
    assert_eq!(
        vela_protocol::canonical::to_canonical_bytes(&decoded_marker).unwrap(),
        marker_bytes
    );
    assert_eq!(marker_bytes.len(), 310);
    assert_eq!(
        ContentDigest::hash(&marker_bytes).as_str(),
        "sha256:651e993f33a99a7759357e975507d297b4ec7520575892c79ed17eb4abd71259"
    );

    let marker_path = temporary.path().join("wire/marker.json");
    operation_journal::write_json(&marker_path, &marker).unwrap();
    let marker_file_bytes = fs::read(&marker_path).unwrap();
    let decoded_marker_file: CommitMarker = operation_journal::read_json(&marker_path).unwrap();
    assert_eq!(decoded_marker_file, marker);
    operation_journal::write_json(&marker_path, &decoded_marker_file).unwrap();
    assert_eq!(fs::read(&marker_path).unwrap(), marker_file_bytes);

    let journal_path = temporary.path().join("wire/journal.json");
    operation_journal::write_json(&journal_path, &journal).unwrap();
    let journal_bytes = fs::read(&journal_path).unwrap();
    let decoded_journal: RepositoryTxnJournal =
        operation_journal::read_json(&journal_path).unwrap();
    assert_eq!(decoded_journal, journal);
    operation_journal::write_json(&journal_path, &decoded_journal).unwrap();
    assert_eq!(fs::read(&journal_path).unwrap(), journal_bytes);
    assert_eq!(journal_bytes.len(), 2681);
    assert_eq!(
        ContentDigest::hash(&journal_bytes).as_str(),
        "sha256:d86b51edbcfcd592e449536e159e121b4de52729a427194cb27c6df8035f91e3"
    );

    assert_eq!(marker_file_bytes.len(), 328);
    assert_eq!(
        ContentDigest::hash(&marker_file_bytes).as_str(),
        "sha256:743789adb9484685ee197f8da04e06a36d475c69b6b21f30d2f7dbcf6479e390"
    );
}

fn initialize_failpoint_repository(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("keep.txt"), b"unchanged").unwrap();
    fs::write(root.join("obsolete.json"), b"remove me").unwrap();
}

fn failpoint_writes() -> Vec<PlannedWrite> {
    vec![
        PlannedWrite::write(
            RepoPath::parse("records/evidence.json").unwrap(),
            WriteClass::CanonicalEvidence,
            b"evidence".to_vec(),
        ),
        PlannedWrite::write(
            RepoPath::parse("records/review/pending.json").unwrap(),
            WriteClass::PublicReview,
            b"pending".to_vec(),
        ),
        PlannedWrite::write(
            RepoPath::parse("records/authority.json").unwrap(),
            WriteClass::Authority,
            b"authority".to_vec(),
        ),
        PlannedWrite::write(
            RepoPath::parse("repository.json").unwrap(),
            WriteClass::CanonicalEvidence,
            b"materialized repository".to_vec(),
        ),
        PlannedWrite::delete(
            RepoPath::parse("obsolete.json").unwrap(),
            WriteClass::PublicReview,
        ),
    ]
}

fn snapshot_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = fs::read_dir(directory)
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(
                !metadata.file_type().is_symlink(),
                "fixture unexpectedly contains a symlink at {}",
                path.display()
            );
            if metadata.is_dir() {
                visit(root, &path, files);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/");
                files.insert(relative, fs::read(path).unwrap());
            }
        }
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn expected_failpoint_postimage() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::from([
        (
            "repository.json".to_string(),
            b"materialized repository".to_vec(),
        ),
        ("keep.txt".to_string(), b"unchanged".to_vec()),
        ("records/authority.json".to_string(), b"authority".to_vec()),
        ("records/evidence.json".to_string(), b"evidence".to_vec()),
        (
            "records/review/pending.json".to_string(),
            b"pending".to_vec(),
        ),
    ])
}

fn assert_injected<T>(result: Result<T, RepositoryTxnError>, expected: RepositoryTxnStep) {
    match result {
        Err(RepositoryTxnError::InjectedFailure { step }) => assert_eq!(step, expected),
        Err(error) => panic!("expected injected failure at {expected:?}, got {error}"),
        Ok(_) => panic!("failpoint {expected:?} was not reached"),
    }
}

fn assert_post_marker_recovery_is_exact(root: &Path, journals: &Path, operation_id: &OperationId) {
    assert!(matches!(
        RepositoryTxn::recover(root, journals, operation_id, TEST_REPOSITORY_ID)
            .unwrap()
            .outcome,
        RecoveryOutcome::Completed | RecoveryOutcome::AlreadyCompleted
    ));
    assert_eq!(snapshot_files(root), expected_failpoint_postimage());
    let first_recovery = snapshot_files(root);
    assert_eq!(
        RepositoryTxn::recover(root, journals, operation_id, TEST_REPOSITORY_ID)
            .unwrap()
            .outcome,
        RecoveryOutcome::AlreadyCompleted
    );
    assert_eq!(
        snapshot_files(root),
        first_recovery,
        "a completed recovery must be byte-idempotent"
    );
}

#[test]
fn repository_txn_rejects_unsafe_paths_and_symlink_ancestors() {
    for path in [
        "",
        "/absolute",
        "../escape",
        "a/../escape",
        "a//b",
        ".git/index",
        "safe\\unsafe",
        "glob/*.json",
        "records/quote\"",
        "records/less<than",
        "records/greater>than",
        "records/pipe|name",
        "records/cafe\u{301}.json",
    ] {
        assert!(
            RepoPath::parse(path).is_err(),
            "accepted unsafe path {path}"
        );
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("linked")).unwrap();
        let error = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("linked/value.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"unsafe".to_vec(),
            )],
        )
        .unwrap_err();
        assert!(matches!(error, RepositoryTxnError::UnsafeTarget { .. }));
    }
}

#[test]
fn open_if_present_returns_none_only_for_an_absent_journal() {
    let (_temp, root, journals) = empty_test_repository();
    let operation_id = OperationId::derive("submission", b"absent request");

    assert!(
        RepositoryTxn::open_if_present(&root, &journals, &operation_id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn open_if_present_exposes_request_identity_and_resumes_marker_window() {
    let (_temp, root, journals) = empty_test_repository();
    let draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("review/pending.json").unwrap(),
            WriteClass::PublicReview,
            b"pending".to_vec(),
        )],
    )
    .unwrap();
    let plan = fixture_plan(&root, &draft, b"exact retry request");
    let operation_id = plan.operation_id.clone();
    let request_root = plan.request_root.clone();
    let result = plan.result.clone();
    let txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    let marker = CommitMarker::from_plan(txn.plan());
    operation_journal::write_json(&txn.paths.marker, &marker).unwrap();
    assert_eq!(txn.recovery_state(), &RecoveryState::Prepared);
    drop(txn);

    let mut reopened = RepositoryTxn::open_if_present(&root, &journals, &operation_id)
        .unwrap()
        .expect("prepared journal");
    assert_eq!(reopened.plan().request_root, request_root);
    assert_eq!(reopened.plan().result, result);
    assert_eq!(reopened.recovery_state(), &RecoveryState::Prepared);
    reopened.mark_committed().unwrap();
    reopened.install().unwrap();
    assert_eq!(
        fs::read(root.join("review/pending.json")).unwrap(),
        b"pending"
    );
}

#[test]
fn canonical_delta_is_sorted_unique_and_root_bound() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repository");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("existing.json"), b"before").unwrap();
    let writes = || {
        vec![
            PlannedWrite::write(
                RepoPath::parse("z.json").unwrap(),
                WriteClass::PublicReview,
                b"z".to_vec(),
            ),
            PlannedWrite::write(
                RepoPath::parse("existing.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"after".to_vec(),
            ),
            PlannedWrite::write(
                RepoPath::parse("a.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"a".to_vec(),
            ),
        ]
    };
    let first = DeltaDraft::prepare(&root, writes()).unwrap();
    assert_eq!(
        first
            .delta
            .writes()
            .iter()
            .map(|write| write.path.as_str())
            .collect::<Vec<_>>(),
        vec!["a.json", "existing.json", "z.json"]
    );
    first.delta.verify().unwrap();

    fs::write(root.join("existing.json"), b"different preimage").unwrap();
    let second = DeltaDraft::prepare(&root, writes()).unwrap();
    assert_ne!(first.delta.root(), second.delta.root());

    let duplicate = DeltaDraft::prepare(
        &root,
        vec![
            PlannedWrite::write(
                RepoPath::parse("same.json").unwrap(),
                WriteClass::PublicReview,
                vec![1],
            ),
            PlannedWrite::write(
                RepoPath::parse("same.json").unwrap(),
                WriteClass::Authority,
                vec![2],
            ),
        ],
    )
    .unwrap_err();
    assert!(matches!(duplicate, RepositoryTxnError::DuplicatePath(_)));

    let portable_collision = DeltaDraft::prepare(
        &root,
        vec![
            PlannedWrite::write(
                RepoPath::parse("records/Foo.json").unwrap(),
                WriteClass::CanonicalEvidence,
                vec![1],
            ),
            PlannedWrite::write(
                RepoPath::parse("records/foo.json").unwrap(),
                WriteClass::CanonicalEvidence,
                vec![2],
            ),
        ],
    )
    .unwrap_err();
    assert!(matches!(
        portable_collision,
        RepositoryTxnError::PortablePathCollision { .. }
    ));
}

#[test]
fn root_consistent_delta_rejects_unbound_postimage_payloads_and_bytes() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repository");
    let journals = temporary.path().join("journals");
    fs::create_dir_all(&root).unwrap();
    let (draft, plan) =
        one_write_fixture(&root, "records/payload-binding.json", b"payload binding");

    let mut unbound = draft.delta.clone();
    unbound.writes[0]
        .payload
        .as_mut()
        .expect("file payload")
        .size += 1;
    unbound.root = CanonicalDelta::compute_root(&unbound.writes).unwrap();
    assert!(matches!(
        unbound.verify(),
        Err(RepositoryTxnError::CorruptPlan(message))
            if message.contains("does not bind its payload digest and size")
    ));

    let mut corrupt = draft.clone();
    let digest = corrupt.delta.writes()[0]
        .payload
        .as_ref()
        .expect("file payload")
        .digest
        .clone();
    corrupt
        .blobs
        .get_mut(&digest)
        .expect("prepared blob")
        .push(0);
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let paths = RepositoryTxnPaths::new(&journals, &plan.operation_id);
    let barrier = RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap();
    let error = RepositoryTxn::prepare_with_recovery_barrier_and_authorization(
        barrier,
        Box::new(RecordingTransactionAuthorization {
            calls: Some(calls.clone()),
            ..Default::default()
        }),
        plan,
        corrupt,
        &mut NoRepositoryTxnFailpoints,
    )
    .unwrap_err();
    assert!(matches!(error, RepositoryTxnError::CorruptBlob(actual) if actual == digest));
    assert!(calls.lock().unwrap().is_empty());
    assert!(!paths.plan.exists());
    assert!(!paths.marker.exists());
    assert!(!paths.blob_dir.exists());
}

#[test]
fn journal_v2_rejects_v1_and_retired_event_fields() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repository");
    fs::create_dir_all(&root).unwrap();
    let draft = DeltaDraft::prepare(&root, vec![]).unwrap();
    let plan = fixture_plan(&root, &draft, b"journal v2 schema");

    let mut retired = serde_json::to_value(&plan).unwrap();
    retired["expected_event_log_root"] = json!(fixture_root('1'));
    assert!(serde_json::from_value::<RepositoryTxnPlan>(retired).is_err());

    let mut v1 = serde_json::to_value(&plan).unwrap();
    v1["schema"] = json!("vela.repository-txn.internal.v1");
    assert!(matches!(
        serde_json::from_value::<RepositoryTxnPlan>(v1)
            .unwrap()
            .verify(),
        Err(RepositoryTxnError::CorruptPlan(message))
            if message.contains("unexpected repository transaction schema")
    ));

    let marker = CommitMarker::from_plan(&plan);
    let mut retired_marker = serde_json::to_value(marker).unwrap();
    retired_marker["resulting_event_log_root"] = json!(fixture_root('2'));
    assert!(serde_json::from_value::<CommitMarker>(retired_marker).is_err());
}

#[test]
fn nested_durable_maps_reject_unknown_fields_before_recovery() {
    type Mutate = fn(&mut serde_json::Value);
    let cases: [(&str, Mutate); 8] = [
        ("repository binding", |journal| {
            journal["plan"]["repository"]["unexpected"] = json!(true);
        }),
        ("input binding", |journal| {
            journal["plan"]["read_set"][0]["unexpected"] = json!(true);
        }),
        ("canonical delta", |journal| {
            journal["plan"]["canonical_delta"]["unexpected"] = json!(true);
        }),
        ("staged write", |journal| {
            journal["plan"]["canonical_delta"]["writes"][0]["unexpected"] = json!(true);
        }),
        ("absent file state", |journal| {
            journal["plan"]["canonical_delta"]["writes"][0]["preimage"]["unexpected"] = json!(true);
        }),
        ("file postimage state", |journal| {
            journal["plan"]["canonical_delta"]["writes"][0]["postimage"]["unexpected"] =
                json!(true);
        }),
        ("journal blob reference", |journal| {
            journal["plan"]["canonical_delta"]["writes"][0]["payload"]["unexpected"] = json!(true);
        }),
        ("prepared recovery state", |journal| {
            journal["recovery"]["unexpected"] = json!(true);
        }),
    ];

    for (label, mutate) in cases {
        let (_temp, root, journals) = empty_test_repository();
        let (operation_id, paths) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/nested-schema.json",
            label.as_bytes(),
        );
        let mut journal: serde_json::Value = operation_journal::read_json(&paths.plan).unwrap();
        mutate(&mut journal);
        operation_journal::write_json(&paths.plan, &journal).unwrap();
        let before = snapshot_files(&journals);

        let error = RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID)
            .expect_err("nested unknown field must fail closed");
        assert!(
            matches!(error, RepositoryTxnError::Journal(ref message) if message.contains("unknown field")),
            "{label} was not rejected as a closed durable map: {error:?}"
        );
        assert_eq!(
            snapshot_files(&journals),
            before,
            "{label} mutated journals"
        );
        assert!(!root.join("records/nested-schema.json").exists());
    }

    for recovery in [
        json!({"state": "installing", "installed": 1, "total": 1, "unexpected": true}),
        json!({
            "state": "committed_conflict",
            "path": "records/nested-schema.json",
            "unexpected": true,
        }),
    ] {
        assert!(serde_json::from_value::<RecoveryState>(recovery).is_err());
    }
}

#[test]
fn pre_marker_failpoints_leave_zero_repository_delta_and_retry_exactly() {
    let blob_count = 4;
    let mut prepare_failpoints = Vec::new();
    for index in 0..blob_count {
        prepare_failpoints.push(RepositoryTxnStep::BeforeBlobJournalWrite { index });
        prepare_failpoints.push(RepositoryTxnStep::AfterBlobJournalWrite { index });
    }
    prepare_failpoints.extend([
        RepositoryTxnStep::BeforePreparedJournalWrite,
        RepositoryTxnStep::AfterPreparedJournalWrite,
    ]);

    for step in prepare_failpoints {
        let (_temp, root, journals) = empty_test_repository();
        initialize_failpoint_repository(&root);
        let before = snapshot_files(&root);
        let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
        assert_eq!(draft.blobs.len(), blob_count);
        let plan = fixture_plan(&root, &draft, format!("prepare {step:?}").as_bytes());
        let operation_id = plan.operation_id.clone();
        let paths = RepositoryTxnPaths::new(&journals, &operation_id);

        assert_injected(
            RepositoryTxn::prepare_at_failpoint(&root, &journals, plan, draft, step),
            step,
        );

        assert_eq!(
            snapshot_files(&root),
            before,
            "pre-marker failpoint {step:?} changed the repository"
        );
        assert!(
            !paths.marker.exists(),
            "pre-marker failpoint {step:?} wrote a commit marker"
        );

        // Partial private blob journals and a fully durable Prepared
        // journal are both safe to retry. Neither is canonical state.
        let retry_draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
        let retry_plan = fixture_plan(&root, &retry_draft, format!("prepare {step:?}").as_bytes());
        let mut retry = if paths.plan.exists() {
            let retry = RepositoryTxn::open(&root, &journals, &operation_id).unwrap();
            assert_eq!(
                retry.plan().root(),
                retry_plan.root(),
                "a durable Prepared journal must bind the exact retry plan"
            );
            retry
        } else {
            RepositoryTxn::prepare(&root, &journals, retry_plan, retry_draft).unwrap()
        };
        if retry.authorization.is_none() {
            retry.bind_exact_test_authorization().unwrap();
        }
        retry.mark_committed().unwrap();
        retry.install().unwrap();
        retry.complete().unwrap();
        drop(retry);
        assert_eq!(snapshot_files(&root), expected_failpoint_postimage());
    }

    // Aborting a marker-free plan is itself a durable journal transition.
    // A failure on either side of that atomic replacement must still leave
    // zero repository delta, no marker, and a retryable operation identity.
    for step in [
        RepositoryTxnStep::BeforeAbortedJournalWrite,
        RepositoryTxnStep::AfterAbortedJournalWrite,
    ] {
        let (_temp, root, journals) = empty_test_repository();
        initialize_failpoint_repository(&root);
        let before = snapshot_files(&root);
        let identity = format!("abort {step:?}");
        let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
        let plan = fixture_plan(&root, &draft, identity.as_bytes());
        let operation_id = plan.operation_id.clone();
        let paths = RepositoryTxnPaths::new(&journals, &operation_id);
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();

        assert_injected(txn.abort_prepared_at_failpoint(step), step);
        assert_eq!(
            snapshot_files(&root),
            before,
            "pre-marker abort failpoint {step:?} changed the repository"
        );
        assert!(
            !paths.marker.exists(),
            "pre-marker abort failpoint {step:?} wrote a commit marker"
        );
        drop(txn);

        let mut reopened = RepositoryTxn::open(&root, &journals, &operation_id).unwrap();
        match step {
            RepositoryTxnStep::BeforeAbortedJournalWrite => {
                assert_eq!(reopened.recovery_state(), &RecoveryState::Prepared);
                reopened.abort_prepared().unwrap();
            }
            RepositoryTxnStep::AfterAbortedJournalWrite => {
                assert_eq!(reopened.recovery_state(), &RecoveryState::Aborted);
            }
            _ => unreachable!(),
        }
        drop(reopened);

        let retry_draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
        let retry_plan = fixture_plan(&root, &retry_draft, identity.as_bytes());
        let mut retry = RepositoryTxn::prepare(&root, &journals, retry_plan, retry_draft).unwrap();
        retry.mark_committed().unwrap();
        retry.install().unwrap();
        retry.complete().unwrap();
        drop(retry);
        assert_eq!(snapshot_files(&root), expected_failpoint_postimage());
    }

    // A safely injected marker-write error occurs before the atomic,
    // fsync-backed journal replacement. The old state is therefore a
    // complete Prepared journal with no marker and no repository delta.
    let (_temp, root, journals) = empty_test_repository();
    initialize_failpoint_repository(&root);
    let before = snapshot_files(&root);
    let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
    let plan = fixture_plan(&root, &draft, b"marker write failure");
    let operation_id = plan.operation_id.clone();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    let step = RepositoryTxnStep::BeforeCommitMarkerWrite;
    assert_injected(txn.mark_committed_at_failpoint(step), step);
    assert_eq!(snapshot_files(&root), before);
    assert!(!txn.paths.marker.exists());
    drop(txn);
    assert_eq!(
        RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID)
            .unwrap()
            .outcome,
        RecoveryOutcome::AbortedPrepared
    );
    assert_eq!(snapshot_files(&root), before);
}

#[test]
fn reused_operation_id_with_changed_request_is_rejected_after_abort_and_completion() {
    for terminal_state in [RecoveryState::Aborted, RecoveryState::Completed] {
        let (_temp, root, journals) = empty_test_repository();
        let identity = format!("operation collision {terminal_state:?}");
        let original_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/original.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"original".to_vec(),
            )],
        )
        .unwrap();
        let original_plan = fixture_plan(&root, &original_draft, identity.as_bytes());
        let operation_id = original_plan.operation_id.clone();
        let mut original =
            RepositoryTxn::prepare(&root, &journals, original_plan, original_draft).unwrap();
        match terminal_state {
            RecoveryState::Aborted => original.abort_prepared().unwrap(),
            RecoveryState::Completed => {
                original.mark_committed().unwrap();
                original.install().unwrap();
                original.complete().unwrap();
            }
            _ => unreachable!(),
        }
        drop(original);

        let changed_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/changed.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"changed".to_vec(),
            )],
        )
        .unwrap();
        let mut changed_plan = fixture_plan(&root, &changed_draft, identity.as_bytes());
        assert_eq!(changed_plan.operation_id, operation_id);
        changed_plan.request_root = ContentDigest::hash(b"different normalized request");
        changed_plan.root = changed_plan.compute_root().unwrap();

        assert!(matches!(
            RepositoryTxn::prepare(&root, &journals, changed_plan, changed_draft),
            Err(RepositoryTxnError::OperationConflict {
                operation_id: conflict
            }) if conflict == operation_id.as_str()
        ));
    }
}

#[test]
fn post_marker_failpoints_recover_the_exact_delta_idempotently() {
    let mut failpoints = vec![
        RepositoryTxnStep::AfterCommitMarkerWrite,
        // This is the durable-marker/Prepared-journal window produced by
        // a committed-journal write failure.
        RepositoryTxnStep::BeforeCommittedJournalWrite,
        RepositoryTxnStep::AfterCommittedJournalWrite,
    ];
    for index in 0..failpoint_writes().len() {
        failpoints.extend([
            RepositoryTxnStep::BeforeInstallWrite { index },
            RepositoryTxnStep::AfterInstallWrite { index },
            RepositoryTxnStep::BeforeInstallingJournalWrite { index },
            RepositoryTxnStep::AfterInstallingJournalWrite { index },
        ]);
    }
    failpoints.extend([
        RepositoryTxnStep::BeforeInstalledJournalWrite,
        RepositoryTxnStep::AfterInstalledJournalWrite,
        RepositoryTxnStep::BeforeInstalledStateVerification,
        RepositoryTxnStep::AfterInstalledStateVerification,
        RepositoryTxnStep::BeforeCompletedJournalWrite,
        RepositoryTxnStep::AfterCompletedJournalWrite,
    ]);

    for step in failpoints {
        let (_temp, root, journals) = empty_test_repository();
        initialize_failpoint_repository(&root);
        let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
        assert!(draft.delta.writes().iter().any(|write| {
            write.class == WriteClass::CanonicalEvidence
                && matches!(write.postimage, FileState::File { .. })
        }));
        assert!(draft.delta.writes().iter().any(|write| {
            write.class == WriteClass::PublicReview && matches!(write.postimage, FileState::Absent)
        }));
        let plan = fixture_plan(&root, &draft, format!("post marker {step:?}").as_bytes());
        let operation_id = plan.operation_id.clone();
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();

        let result = match step {
            RepositoryTxnStep::AfterCommitMarkerWrite
            | RepositoryTxnStep::BeforeCommittedJournalWrite
            | RepositoryTxnStep::AfterCommittedJournalWrite => {
                txn.mark_committed_at_failpoint(step)
            }
            RepositoryTxnStep::BeforeInstallWrite { .. }
            | RepositoryTxnStep::AfterInstallWrite { .. }
            | RepositoryTxnStep::BeforeInstallingJournalWrite { .. }
            | RepositoryTxnStep::AfterInstallingJournalWrite { .. }
            | RepositoryTxnStep::BeforeInstalledJournalWrite
            | RepositoryTxnStep::AfterInstalledJournalWrite => {
                txn.mark_committed().unwrap();
                txn.install_at_failpoint(step)
            }
            RepositoryTxnStep::BeforeInstalledStateVerification
            | RepositoryTxnStep::AfterInstalledStateVerification
            | RepositoryTxnStep::BeforeCompletedJournalWrite
            | RepositoryTxnStep::AfterCompletedJournalWrite => {
                txn.mark_committed().unwrap();
                txn.install().unwrap();
                txn.complete_at_failpoint(step)
            }
            _ => unreachable!("not a post-marker failpoint: {step:?}"),
        };
        assert_injected(result, step);
        assert!(
            txn.paths.marker.exists(),
            "post-marker failpoint {step:?} lost the commit marker"
        );
        drop(txn);

        assert_post_marker_recovery_is_exact(&root, &journals, &operation_id);
    }
}

#[test]
fn committed_conflict_journal_failpoints_preserve_drift_and_recover_after_repair() {
    for index in 0..failpoint_writes().len() {
        for step in [
            RepositoryTxnStep::BeforeCommittedConflictJournalWrite { index },
            RepositoryTxnStep::AfterCommittedConflictJournalWrite { index },
        ] {
            let (_temp, root, journals) = empty_test_repository();
            initialize_failpoint_repository(&root);
            let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
            let conflicted_write = draft.delta.writes()[index].clone();
            let conflicted_target = conflicted_write.path.target(&root).unwrap();
            let original_bytes = fs::read(&conflicted_target).ok();
            let plan = fixture_plan(&root, &draft, format!("conflict {step:?}").as_bytes());
            let operation_id = plan.operation_id.clone();
            let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
            txn.mark_committed().unwrap();

            fs::create_dir_all(conflicted_target.parent().unwrap()).unwrap();
            fs::write(&conflicted_target, b"third-party drift").unwrap();
            assert_injected(txn.install_at_failpoint(step), step);
            assert_eq!(
                fs::read(&conflicted_target).unwrap(),
                b"third-party drift",
                "conflict failpoint {step:?} overwrote external drift"
            );
            assert!(txn.paths.marker.exists());
            drop(txn);

            assert!(matches!(
                RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID),
                Err(RepositoryTxnError::CommittedConflict { path, .. })
                    if path == conflicted_write.path
            ));
            assert_eq!(fs::read(&conflicted_target).unwrap(), b"third-party drift");

            match original_bytes {
                Some(bytes) => fs::write(&conflicted_target, bytes).unwrap(),
                None => fs::remove_file(&conflicted_target).unwrap(),
            }
            assert_post_marker_recovery_is_exact(&root, &journals, &operation_id);
        }
    }
}

#[test]
fn committed_install_is_idempotent_and_recovers_after_failpoint() {
    let (_temp, root, journals) = empty_test_repository();
    let draft = DeltaDraft::prepare(
        &root,
        vec![
            PlannedWrite::write(
                RepoPath::parse("records/receipt.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"receipt".to_vec(),
            ),
            PlannedWrite::write(
                RepoPath::parse("repository.json").unwrap(),
                WriteClass::PublicReview,
                b"repository".to_vec(),
            ),
        ],
    )
    .unwrap();
    let plan = fixture_plan(&root, &draft, b"recoverable request");
    let operation_id = plan.operation_id.clone();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    txn.mark_committed().unwrap();
    let step = RepositoryTxnStep::AfterInstallingJournalWrite { index: 0 };
    let error = txn.install_at_failpoint(step).unwrap_err();
    assert!(matches!(
        error,
        RepositoryTxnError::InjectedFailure { step: actual } if actual == step
    ));
    drop(txn);

    let recovery =
        RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID).unwrap();
    assert_eq!(
        recovery.prior_state,
        RecoveryState::Installing {
            installed: 1,
            total: 2,
        }
    );
    assert_eq!(recovery.outcome, RecoveryOutcome::Completed);
    let reopened = RepositoryTxn::open(&root, &journals, &operation_id).unwrap();
    assert_eq!(reopened.recovery_state(), &RecoveryState::Completed);
    drop(reopened);
    assert_eq!(
        fs::read(root.join("records/receipt.json")).unwrap(),
        b"receipt"
    );
    assert_eq!(
        fs::read(root.join("repository.json")).unwrap(),
        b"repository"
    );
    assert_eq!(
        RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID)
            .unwrap()
            .outcome,
        RecoveryOutcome::AlreadyCompleted,
        "replaying a completed transaction must remain idempotent"
    );
}

#[test]
fn completed_recovery_blob_retirement_preserves_plan_marker_and_replay() {
    let (_temp, root, journals) = empty_test_repository();
    let draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("records/submission.json").unwrap(),
            WriteClass::PublicReview,
            b"published submission".to_vec(),
        )],
    )
    .unwrap();
    let plan = fixture_plan(&root, &draft, b"retire completed recovery blobs");
    let operation_id = plan.operation_id.clone();
    let paths = RepositoryTxnPaths::new(&journals, &operation_id);
    let blob = draft.delta.writes()[0]
        .payload
        .as_ref()
        .unwrap()
        .digest
        .clone();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    txn.mark_committed().unwrap();
    txn.install().unwrap();
    txn.complete().unwrap();

    assert!(paths.plan.is_file());
    assert!(paths.marker.is_file());
    assert!(paths.blob(&blob).is_file());
    assert_eq!(txn.retire_completed_recovery_blobs().unwrap(), 1);
    assert!(paths.plan.is_file(), "the durable plan must remain");
    assert!(paths.marker.is_file(), "the commit marker must remain");
    assert!(!paths.blob(&blob).exists());
    let retained: RepositoryTxnJournal = operation_journal::read_json(&paths.plan).unwrap();
    assert_eq!(retained.recovery, RecoveryState::Completed);
    assert_eq!(retained.blob_retention, BlobRetention::Pruned);
    drop(txn);

    let reopened = RepositoryTxn::open(&root, &journals, &operation_id).unwrap();
    assert_eq!(reopened.recovery_state(), &RecoveryState::Completed);
    assert_eq!(
        reopened.resolved_public_writes().unwrap()[0]
            .postimage_bytes
            .as_deref(),
        Some(b"published submission".as_slice())
    );
    drop(reopened);
    assert_eq!(
        RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID)
            .unwrap()
            .outcome,
        RecoveryOutcome::AlreadyCompleted
    );
}

#[test]
fn recovery_blobs_survive_a_crash_until_explicit_completed_retirement() {
    let (_temp, root, journals) = empty_test_repository();
    let draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("records/verification.json").unwrap(),
            WriteClass::PublicReview,
            b"verified bytes".to_vec(),
        )],
    )
    .unwrap();
    let plan = fixture_plan(&root, &draft, b"crash before completed retirement");
    let operation_id = plan.operation_id.clone();
    let paths = RepositoryTxnPaths::new(&journals, &operation_id);
    let blob = draft.delta.writes()[0]
        .payload
        .as_ref()
        .unwrap()
        .digest
        .clone();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    txn.mark_committed().unwrap();
    let step = RepositoryTxnStep::AfterInstallWrite { index: 0 };
    assert!(matches!(
        txn.install_at_failpoint(step),
        Err(RepositoryTxnError::InjectedFailure { step: actual }) if actual == step
    ));
    assert!(paths.blob(&blob).is_file());
    assert!(matches!(
        txn.retire_completed_recovery_blobs(),
        Err(RepositoryTxnError::CorruptPlan(message))
            if message.contains("cannot retire recovery blobs")
    ));
    assert!(paths.blob(&blob).is_file());
    drop(txn);

    assert_eq!(
        RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID)
            .unwrap()
            .outcome,
        RecoveryOutcome::Completed
    );
    assert!(paths.blob(&blob).is_file());
    let mut recovered = RepositoryTxn::open(&root, &journals, &operation_id).unwrap();
    assert_eq!(recovered.retire_completed_recovery_blobs().unwrap(), 1);
    assert!(!paths.blob(&blob).exists());
}

#[test]
fn shared_blob_is_removed_only_after_every_referencing_journal_is_pruned() {
    let (_temp, root, journals) = empty_test_repository();

    let first_draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("records/first.json").unwrap(),
            WriteClass::PublicReview,
            b"shared recovery bytes".to_vec(),
        )],
    )
    .unwrap();
    let shared_blob = first_draft.delta.writes()[0]
        .payload
        .as_ref()
        .unwrap()
        .digest
        .clone();
    let first_plan = fixture_plan(&root, &first_draft, b"first shared blob journal");
    let first_operation = first_plan.operation_id.clone();
    let first_paths = RepositoryTxnPaths::new(&journals, &first_operation);
    let mut first = RepositoryTxn::prepare(&root, &journals, first_plan, first_draft).unwrap();
    first.mark_committed().unwrap();
    first.install().unwrap();
    first.complete().unwrap();
    drop(first);

    let second_draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("records/second.json").unwrap(),
            WriteClass::PublicReview,
            b"shared recovery bytes".to_vec(),
        )],
    )
    .unwrap();
    assert_eq!(
        second_draft.delta.writes()[0]
            .payload
            .as_ref()
            .unwrap()
            .digest,
        shared_blob
    );
    let second_plan = fixture_plan(&root, &second_draft, b"second shared blob journal");
    let second_operation = second_plan.operation_id.clone();
    let second_paths = RepositoryTxnPaths::new(&journals, &second_operation);
    let mut second = RepositoryTxn::prepare(&root, &journals, second_plan, second_draft).unwrap();
    second.mark_committed().unwrap();
    second.install().unwrap();
    second.complete().unwrap();

    assert_eq!(second.retire_completed_recovery_blobs().unwrap(), 0);
    assert!(second_paths.blob(&shared_blob).is_file());
    drop(second);

    let mut first = RepositoryTxn::open(&root, &journals, &first_operation).unwrap();
    assert_eq!(first.retire_completed_recovery_blobs().unwrap(), 1);
    assert!(!first_paths.blob(&shared_blob).exists());
    drop(first);

    let second = RepositoryTxn::open(&root, &journals, &second_operation).unwrap();
    assert_eq!(second.recovery_state(), &RecoveryState::Completed);
}

#[test]
fn incomplete_journal_is_a_repository_wide_recovery_barrier() {
    let (_temp, root, journals) = empty_test_repository();
    let first_draft = DeltaDraft::prepare(
        &root,
        vec![
            PlannedWrite::write(
                RepoPath::parse("records/first.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"first".to_vec(),
            ),
            PlannedWrite::write(
                RepoPath::parse("repository.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"first repository".to_vec(),
            ),
        ],
    )
    .unwrap();
    let first_plan = fixture_plan(&root, &first_draft, b"first operation");
    let first_operation = first_plan.operation_id.clone();
    let mut first = RepositoryTxn::prepare(&root, &journals, first_plan, first_draft).unwrap();
    first.mark_committed().unwrap();
    let step = RepositoryTxnStep::AfterInstallingJournalWrite { index: 0 };
    assert!(matches!(
        first.install_at_failpoint(step),
        Err(RepositoryTxnError::InjectedFailure { step: actual }) if actual == step
    ));
    drop(first);

    let barrier_error = RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap_err();
    assert!(matches!(
        barrier_error,
        RepositoryTxnError::RecoveryRequired {
            operation_id,
            state: RecoveryState::Installing {
                installed: 1,
                total: 2
            }
        } if operation_id == first_operation.as_str()
    ));
    assert!(matches!(
        RepositoryTxn::verify_recovery_barrier(&root, &journals),
        Err(RepositoryTxnError::RecoveryRequired { operation_id, .. })
            if operation_id == first_operation.as_str()
    ));

    let second_draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("records/second.json").unwrap(),
            WriteClass::CanonicalEvidence,
            b"second".to_vec(),
        )],
    )
    .unwrap();
    let second_plan = fixture_plan(&root, &second_draft, b"second operation");
    assert!(matches!(
        RepositoryTxn::prepare(&root, &journals, second_plan, second_draft),
        Err(RepositoryTxnError::RecoveryRequired { operation_id, .. })
            if operation_id == first_operation.as_str()
    ));

    assert_eq!(
        RepositoryTxn::recover(&root, &journals, &first_operation, TEST_REPOSITORY_ID)
            .unwrap()
            .outcome,
        RecoveryOutcome::Completed
    );
    let barrier = RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap();
    drop(barrier);
    RepositoryTxn::verify_recovery_barrier(&root, &journals).unwrap();
}

#[test]
fn recovery_barrier_diagnostic_is_read_only_even_when_blocked() {
    let (_temp, root, journals) = empty_test_repository();
    RepositoryTxn::verify_recovery_barrier(&root, &journals).unwrap();
    assert!(!journals.exists());

    let (operation_id, _) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/pending-diagnostic.json",
        b"read-only recovery diagnostic",
    );
    let lock_dir = journals.join("repository-locks");
    fs::remove_dir_all(&lock_dir).unwrap();
    assert!(matches!(
        RepositoryTxn::verify_recovery_barrier(&root, &journals),
        Err(RepositoryTxnError::RecoveryRequired {
            operation_id: blocked,
            state: RecoveryState::Prepared,
        }) if blocked == operation_id.as_str()
    ));
    assert!(
        !lock_dir.exists(),
        "a read-only diagnostic must not recreate the recovery lock"
    );
    assert!(!root.join("records/pending-diagnostic.json").exists());
}

#[test]
fn commit_marker_index_rejects_invalid_orphan_and_nonregular_semantic_entries() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repository");
    let journals = temp.path().join("journals");
    let marker_dir = journals.join("repository/committed");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&marker_dir).unwrap();

    fs::write(
        marker_dir.join(TEST_OWNED_ATOMIC_TEMP),
        b"temporary residue",
    )
    .unwrap();
    RepositoryTxn::verify_recovery_barrier(&root, &journals).unwrap();

    let unowned_temp = marker_dir.join(".marker-write.tmp");
    fs::write(&unowned_temp, b"unowned temporary residue").unwrap();
    assert!(matches!(
        RepositoryTxn::verify_recovery_barrier(&root, &journals),
        Err(RepositoryTxnError::Journal(error))
            if error.contains("unexpected repository commit-marker entry")
    ));
    fs::remove_file(&unowned_temp).unwrap();

    let invalid = marker_dir.join("evil.json");
    fs::write(&invalid, b"{}").unwrap();
    assert!(matches!(
        RepositoryTxn::verify_recovery_barrier(&root, &journals),
        Err(RepositoryTxnError::InvalidOperationId(value)) if value == "evil"
    ));
    fs::remove_file(&invalid).unwrap();

    let (draft, plan) = one_write_fixture(&root, "records/orphan.json", b"orphan marker");
    assert_eq!(draft.delta, plan.canonical_delta);
    let orphan_path = RepositoryTxnPaths::new(&journals, &plan.operation_id).marker;
    operation_journal::write_json(&orphan_path, &CommitMarker::from_plan(&plan)).unwrap();
    assert!(matches!(
        RepositoryTxn::verify_recovery_barrier(&root, &journals),
        Err(RepositoryTxnError::CorruptPlan(error))
            if error.contains("has no matching durable plan")
    ));

    fs::remove_file(&orphan_path).unwrap();
    fs::create_dir(&orphan_path).unwrap();
    assert!(matches!(
        RepositoryTxn::verify_recovery_barrier(&root, &journals),
        Err(RepositoryTxnError::Journal(error))
            if error.contains("not a regular non-symlink file")
    ));
    fs::remove_dir(&orphan_path).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let outside = temp.path().join("outside-marker.json");
        fs::write(&outside, b"{}").unwrap();
        symlink(&outside, &orphan_path).unwrap();
        assert!(matches!(
            RepositoryTxn::verify_recovery_barrier(&root, &journals),
            Err(RepositoryTxnError::Journal(error))
                if error.contains("not a regular non-symlink file")
        ));
    }
}

#[test]
fn private_recovery_inventory_rejects_unowned_aliases_and_kind_substitution() {
    fn assert_rejected(label: &str, mutate: impl FnOnce(&Path, &Path)) -> RepositoryTxnError {
        let (_temp, root, journals) = empty_test_repository();
        drop(RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap());
        mutate(&root, &journals);
        let error = RepositoryTxn::verify_recovery_barrier(&root, &journals)
            .expect_err("hostile private residue must fail closed");
        assert!(
            !matches!(
                error,
                RepositoryTxnError::Busy | RepositoryTxnError::RecoveryRequired { .. }
            ),
            "{label} produced an unsafe operational hint: {error:?}"
        );
        error
    }

    for name in [
        "arbitrary-residue",
        ".journal-write.tmp",
        ".Vela-journal-tmp-abcdefghijkl",
        ".vela-journal-tmp-abcdefghijké",
        ".vela-journal-tmp-abcdefghijkl.json",
    ] {
        let error = assert_rejected(name, |_, journals| {
            let repository = journals.join("repository");
            fs::create_dir_all(&repository).unwrap();
            fs::write(repository.join(name), b"unowned").unwrap();
        });
        assert!(matches!(error, RepositoryTxnError::Journal(_)));
    }

    for name in ["Repository", "repository-locks-copy"] {
        let error = assert_rejected(name, |_, journals| {
            fs::create_dir(journals.join(name)).unwrap();
        });
        assert!(matches!(error, RepositoryTxnError::Journal(_)));
    }

    let error = assert_rejected("case-aliased blob directory", |_, journals| {
        fs::create_dir_all(journals.join("repository/Blobs")).unwrap();
    });
    assert!(matches!(error, RepositoryTxnError::Journal(_)));

    let error = assert_rejected("foreign repository lock", |_, journals| {
        fs::write(journals.join("repository-locks/foreign.lock"), b"").unwrap();
    });
    assert!(matches!(error, RepositoryTxnError::Journal(_)));

    let error = assert_rejected("nonempty repository lock", |root, journals| {
        let canonical_root = canonical_repository_root(root).unwrap();
        fs::write(
            RepositoryWriteLock::path(journals, &canonical_root),
            b"not a lock file",
        )
        .unwrap();
    });
    assert!(matches!(error, RepositoryTxnError::Journal(_)));

    let error = assert_rejected("malformed unreferenced blob", |_, journals| {
        let blob_dir = journals.join("repository/blobs");
        fs::create_dir_all(&blob_dir).unwrap();
        let bytes = b"unreferenced malformed blob";
        let digest = ContentDigest::hash(bytes);
        fs::write(
            blob_dir.join(format!("{}.json", digest.file_stem())),
            serde_json::to_vec(&json!({
                "schema": REPOSITORY_TXN_BLOB_SCHEMA,
                "digest": digest.as_str(),
                "size": bytes.len(),
                "bytes": bytes,
                "unexpected": true,
            }))
            .unwrap(),
        )
        .unwrap();
    });
    assert!(matches!(error, RepositoryTxnError::Journal(_)));

    let error = assert_rejected("owned-temp kind substitution", |_, journals| {
        fs::create_dir_all(
            journals
                .join("repository/blobs")
                .join(TEST_OWNED_ATOMIC_TEMP),
        )
        .unwrap();
    });
    assert!(matches!(error, RepositoryTxnError::Journal(_)));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let error = assert_rejected("owned-temp symlink substitution", |_, journals| {
            let blob_dir = journals.join("repository/blobs");
            fs::create_dir_all(&blob_dir).unwrap();
            symlink("outside", blob_dir.join(TEST_OWNED_ATOMIC_TEMP)).unwrap();
        });
        assert!(matches!(error, RepositoryTxnError::Journal(_)));
    }
}

#[test]
fn completed_journal_fails_closed_when_a_postimage_is_missing() {
    let (_temp, root, journals) = empty_test_repository();
    let draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("records/receipt.json").unwrap(),
            WriteClass::CanonicalEvidence,
            b"receipt".to_vec(),
        )],
    )
    .unwrap();
    let plan = fixture_plan(&root, &draft, b"completed corruption");
    let operation_id = plan.operation_id.clone();
    mark_install_complete(RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap());

    fs::remove_file(root.join("records/receipt.json")).unwrap();
    assert!(matches!(
        RepositoryTxn::open_if_present(&root, &journals, &operation_id),
        Err(RepositoryTxnError::CompletedPostimageMismatch {
            operation_id: corrupt_operation,
            ..
        }) if corrupt_operation == operation_id.as_str()
    ));
    assert!(matches!(
        RepositoryTxn::acquire_recovery_barrier(&root, &journals),
        Err(RepositoryTxnError::CompletedPostimageMismatch { .. })
    ));
}

#[test]
fn completed_history_rejects_out_of_transaction_head_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repository");
    let journals = temp.path().join("journals");
    fs::create_dir_all(root.join(".vela")).unwrap();

    let draft = DeltaDraft::prepare(
        &root,
        vec![
            PlannedWrite::write(
                RepoPath::parse(".vela/repository.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"authenticated repository head one".to_vec(),
            ),
            PlannedWrite::write(
                RepoPath::parse("records/receipt.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"immutable receipt".to_vec(),
            ),
        ],
    )
    .unwrap();
    let plan = fixture_plan(&root, &draft, b"rolling repository head");
    mark_install_complete(RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap());

    fs::write(
        root.join(".vela/repository.json"),
        b"out-of-transaction repository head",
    )
    .unwrap();
    assert!(matches!(
        RepositoryTxn::acquire_recovery_barrier(&root, &journals),
        Err(RepositoryTxnError::CompletedPostimageMismatch { path, .. })
            if path.as_str() == ".vela/repository.json"
    ));
}

#[test]
fn completed_history_proves_multi_step_superseded_postimages_out_of_order() {
    let (_temp, root, journals) = empty_test_repository();

    let first_draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse(".vela/repository.json").unwrap(),
            WriteClass::CanonicalEvidence,
            b"first head".to_vec(),
        )],
    )
    .unwrap();
    let first_plan = fixture_plan(&root, &first_draft, b"first neutral operation");
    let first_operation = first_plan.operation_id.clone();
    let first = RepositoryTxn::prepare(&root, &journals, first_plan, first_draft).unwrap();
    mark_install_complete(first);

    let second_draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse(".vela/repository.json").unwrap(),
            WriteClass::CanonicalEvidence,
            b"second head".to_vec(),
        )],
    )
    .unwrap();
    let second_plan = fixture_plan(&root, &second_draft, b"second neutral operation");
    let second = RepositoryTxn::prepare(&root, &journals, second_plan, second_draft).unwrap();
    mark_install_complete(second);

    let third_draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse(".vela/repository.json").unwrap(),
            WriteClass::CanonicalEvidence,
            b"third head".to_vec(),
        )],
    )
    .unwrap();
    let third_plan = fixture_plan(&root, &third_draft, b"third neutral operation");
    let third = RepositoryTxn::prepare(&root, &journals, third_plan, third_draft).unwrap();
    mark_install_complete(third);

    let mut completed = repository_journals(&root, &journals).unwrap();
    completed.reverse();
    verify_completed_history(&root, &completed).unwrap();

    let first_retry = RepositoryTxn::open(&root, &journals, &first_operation).unwrap();
    assert_eq!(first_retry.recovery_state(), &RecoveryState::Completed);
    drop(first_retry);
    drop(RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap());
}

#[test]
fn completed_history_rejects_corrupt_marker_and_blob() {
    let (_temp, root, journals) = empty_test_repository();
    let draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("records/receipt.json").unwrap(),
            WriteClass::CanonicalEvidence,
            b"receipt".to_vec(),
        )],
    )
    .unwrap();
    let plan = fixture_plan(&root, &draft, b"corrupt durable history");
    let operation_id = plan.operation_id.clone();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    txn.mark_committed().unwrap();
    txn.install().unwrap();
    let completion_step = RepositoryTxnStep::AfterCompletedJournalWrite;
    assert!(matches!(
        txn.complete_at_failpoint(completion_step),
        Err(RepositoryTxnError::InjectedFailure { step }) if step == completion_step
    ));
    let marker_path = txn.paths.marker.clone();
    let blob_path = txn.paths.blob(
        &txn.plan()
            .canonical_delta
            .writes()
            .first()
            .unwrap()
            .payload
            .as_ref()
            .unwrap()
            .digest,
    );
    drop(txn);

    let marker_bytes = fs::read(&marker_path).unwrap();
    fs::write(&marker_path, b"{}").unwrap();
    assert!(RepositoryTxn::open(&root, &journals, &operation_id).is_err());
    fs::write(&marker_path, marker_bytes).unwrap();

    let blob_bytes = fs::read(&blob_path).unwrap();
    let mut corrupt_blob: serde_json::Value = serde_json::from_slice(&blob_bytes).unwrap();
    corrupt_blob["bytes"] = json!([0, 1, 2, 3]);
    fs::write(
        &blob_path,
        serde_json::to_vec_pretty(&corrupt_blob).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        RepositoryTxn::open(&root, &journals, &operation_id),
        Err(RepositoryTxnError::CorruptBlob(_))
    ));
}

#[test]
fn committed_install_never_overwrites_post_marker_drift() {
    let (_temp, root, journals) = empty_test_repository();
    fs::write(root.join("state.json"), b"before").unwrap();
    let draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("state.json").unwrap(),
            WriteClass::PublicReview,
            b"after".to_vec(),
        )],
    )
    .unwrap();
    let plan = fixture_plan(&root, &draft, b"conflicting request");
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    txn.mark_committed().unwrap();
    fs::write(root.join("state.json"), b"third party drift").unwrap();

    let error = txn.install().unwrap_err();

    assert!(matches!(
        error,
        RepositoryTxnError::CommittedConflict { .. }
    ));
    assert_eq!(
        fs::read(root.join("state.json")).unwrap(),
        b"third party drift"
    );
}

#[cfg(unix)]
#[test]
fn observed_parent_symlink_swap_after_marker_never_writes_outside_the_repository() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repository");
    let journals = temp.path().join("journals");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("records/receipt.json").unwrap(),
            WriteClass::CanonicalEvidence,
            b"receipt".to_vec(),
        )],
    )
    .unwrap();
    let plan = fixture_plan(&root, &draft, b"observed parent symlink swap");
    let operation_id = plan.operation_id.clone();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    txn.mark_committed().unwrap();

    symlink(&outside, root.join("records")).unwrap();
    assert!(matches!(
        txn.install(),
        Err(RepositoryTxnError::UnsafeTarget { .. })
    ));
    assert!(
        !outside.join("receipt.json").exists(),
        "an observed stable symlink substitution must not redirect the write"
    );
    drop(txn);

    // This proves rejection when the substitution is visible to a path
    // check. It intentionally does not claim to eliminate a concurrent
    // hostile-local race between that check and a std::fs path operation;
    // `validate_target` documents that remaining permission boundary.
    fs::remove_file(root.join("records")).unwrap();
    assert_eq!(
        RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID)
            .unwrap()
            .outcome,
        RecoveryOutcome::Completed
    );
    assert_eq!(
        fs::read(root.join("records/receipt.json")).unwrap(),
        b"receipt"
    );
}

#[test]
fn recovery_before_marker_has_zero_repository_delta() {
    let (_temp, root, journals) = empty_test_repository();
    let draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("pending.json").unwrap(),
            WriteClass::PublicReview,
            b"pending".to_vec(),
        )],
    )
    .unwrap();
    let plan = fixture_plan(&root, &draft, b"prepared only");
    let operation_id = plan.operation_id.clone();
    let txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    drop(txn);

    let result =
        RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID).unwrap();
    assert_eq!(result.operation_id, operation_id);
    assert_eq!(result.repository_id, "33333333-3333-4333-8333-333333333333");
    assert_eq!(result.prior_state, RecoveryState::Prepared);
    assert_eq!(result.outcome, RecoveryOutcome::AbortedPrepared);
    assert_eq!(result.next_operation_id, None);
    assert!(!root.join("pending.json").exists());
    assert_eq!(
        RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID)
            .unwrap()
            .outcome,
        RecoveryOutcome::AlreadyAborted
    );
}

#[test]
fn durable_recovery_revalidates_every_path_before_blob_lookup_or_install() {
    for hostile_path in [
        "../outside-sentinel",
        "/absolute-sentinel",
        ".git/config",
        "records/cafe\u{301}.json",
        "records/control\n.json",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let journals = temp.path().join("journals");
        fs::create_dir_all(root.join(".git")).unwrap();
        let outside = temp.path().join("outside-sentinel");
        fs::write(&outside, b"outside-safe").unwrap();
        fs::write(root.join(".git/config"), b"git-safe").unwrap();

        let (operation_id, paths) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/original.json",
            hostile_path.as_bytes(),
        );
        let mut journal: RepositoryTxnJournal = operation_journal::read_json(&paths.plan).unwrap();
        let durable_path = if hostile_path == "/absolute-sentinel" {
            temp.path()
                .join("absolute-sentinel")
                .to_string_lossy()
                .into_owned()
        } else {
            hostile_path.to_string()
        };
        journal.plan.canonical_delta.writes[0].path = RepoPath(durable_path.clone());
        journal.plan.canonical_delta.root =
            CanonicalDelta::compute_root(&journal.plan.canonical_delta.writes).unwrap();
        journal.plan.root = journal.plan.compute_root().unwrap();
        operation_journal::write_json(&paths.plan, &journal).unwrap();
        operation_journal::write_json(&paths.marker, &CommitMarker::from_plan(&journal.plan))
            .unwrap();

        assert!(matches!(
            RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID),
            Err(RepositoryTxnError::InvalidPath { path, .. }) if path == durable_path
        ));
        assert_eq!(fs::read(&outside).unwrap(), b"outside-safe");
        assert_eq!(fs::read(root.join(".git/config")).unwrap(), b"git-safe");
        assert!(!temp.path().join("absolute-sentinel").exists());
        assert!(!root.join("records/original.json").exists());
    }
}

#[test]
fn durable_recovery_rejects_invalid_plan_marker_and_blob_digests_without_panicking() {
    let invalid = ContentDigest("not-a-content-digest".into());

    {
        let (_temp, root, journals) = empty_test_repository();
        let (operation_id, paths) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/invalid-postimage-digest.json",
            b"invalid postimage digest",
        );
        let mut journal: RepositoryTxnJournal = operation_journal::read_json(&paths.plan).unwrap();
        let write = &mut journal.plan.canonical_delta.writes[0];
        if let FileState::File { digest, .. } = &mut write.postimage {
            *digest = invalid.clone();
        }
        write.payload.as_mut().unwrap().digest = invalid.clone();
        journal.plan.canonical_delta.root =
            CanonicalDelta::compute_root(&journal.plan.canonical_delta.writes).unwrap();
        journal.plan.root = journal.plan.compute_root().unwrap();
        operation_journal::write_json(&paths.plan, &journal).unwrap();
        operation_journal::write_json(&paths.marker, &CommitMarker::from_plan(&journal.plan))
            .unwrap();
        assert!(matches!(
            RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID),
            Err(RepositoryTxnError::InvalidDigest(value)) if value == invalid.as_str()
        ));
    }

    {
        let (_temp, root, journals) = empty_test_repository();
        let (operation_id, paths) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/invalid-request-digest.json",
            b"invalid request digest",
        );
        let mut journal: RepositoryTxnJournal = operation_journal::read_json(&paths.plan).unwrap();
        journal.plan.request_root = invalid.clone();
        journal.plan.root = journal.plan.compute_root().unwrap();
        operation_journal::write_json(&paths.plan, &journal).unwrap();
        assert!(matches!(
            RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID),
            Err(RepositoryTxnError::InvalidDigest(value)) if value == invalid.as_str()
        ));
    }

    {
        let (_temp, root, journals) = empty_test_repository();
        let (operation_id, paths) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/invalid-marker-digest.json",
            b"invalid marker digest",
        );
        let journal: RepositoryTxnJournal = operation_journal::read_json(&paths.plan).unwrap();
        let mut marker = CommitMarker::from_plan(&journal.plan);
        marker.plan_root = invalid.clone();
        operation_journal::write_json(&paths.marker, &marker).unwrap();
        assert!(matches!(
            RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID),
            Err(RepositoryTxnError::InvalidDigest(value)) if value == invalid.as_str()
        ));
    }

    {
        let (_temp, root, journals) = empty_test_repository();
        let (draft, plan) = one_write_fixture(
            &root,
            "records/invalid-blob-digest.json",
            b"invalid blob digest",
        );
        let operation_id = plan.operation_id.clone();
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        let paths = txn.paths.clone();
        let blob_ref = txn.journal.plan.canonical_delta.writes[0]
            .payload
            .clone()
            .unwrap();
        drop(txn);
        let blob_path = paths.blob(&blob_ref.digest);
        let mut blob: BlobJournal = operation_journal::read_json(&blob_path).unwrap();
        blob.digest = invalid.clone();
        operation_journal::write_json(&blob_path, &blob).unwrap();
        assert!(matches!(
            RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID),
            Err(RepositoryTxnError::InvalidDigest(value)) if value == invalid.as_str()
        ));
    }
}

#[test]
fn durable_recovery_rejects_impossible_progress_before_repository_mutation() {
    let (_temp, root, journals) = empty_test_repository();
    initialize_failpoint_repository(&root);
    let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
    let plan = fixture_plan(&root, &draft, b"invalid progress shape");
    let operation_id = plan.operation_id.clone();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    txn.mark_committed().unwrap();
    let paths = txn.paths.clone();
    drop(txn);
    let before = snapshot_files(&root);
    let original: RepositoryTxnJournal = operation_journal::read_json(&paths.plan).unwrap();

    for recovery in [
        RecoveryState::Installing {
            installed: 0,
            total: original.plan.canonical_delta.writes().len(),
        },
        RecoveryState::Installing {
            installed: 1,
            total: original.plan.canonical_delta.writes().len() + 1,
        },
        RecoveryState::Installing {
            installed: original.plan.canonical_delta.writes().len() + 1,
            total: original.plan.canonical_delta.writes().len(),
        },
        RecoveryState::CommittedConflict {
            path: RepoPath("../outside".into()),
        },
    ] {
        let mut journal = original.clone();
        journal.recovery = recovery;
        operation_journal::write_json(&paths.plan, &journal).unwrap();
        assert!(matches!(
            RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID),
            Err(RepositoryTxnError::CorruptPlan(_) | RepositoryTxnError::InvalidPath { .. })
        ));
        assert_eq!(snapshot_files(&root), before);
    }
}

#[test]
fn durable_recovery_rejects_rolled_back_or_out_of_order_installation() {
    {
        let (_temp, root, journals) = empty_test_repository();
        let (draft, plan) =
            one_write_fixture(&root, "records/installed.json", b"installed rollback");
        let operation_id = plan.operation_id.clone();
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        txn.install().unwrap();
        assert_eq!(txn.recovery_state(), &RecoveryState::Installed);
        fs::remove_file(root.join("records/installed.json")).unwrap();
        drop(txn);

        assert!(matches!(
            RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID),
            Err(RepositoryTxnError::CompletedPostimageMismatch { path, .. })
                if path.as_str() == "records/installed.json"
        ));
        assert!(!root.join("records/installed.json").exists());
    }

    {
        let (_temp, root, journals) = empty_test_repository();
        initialize_failpoint_repository(&root);
        let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
        let plan = fixture_plan(&root, &draft, b"install prefix rollback");
        let operation_id = plan.operation_id.clone();
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        assert_injected(
            txn.install_at_failpoint(RepositoryTxnStep::AfterInstallingJournalWrite { index: 0 }),
            RepositoryTxnStep::AfterInstallingJournalWrite { index: 0 },
        );
        let first = txn.journal.plan.canonical_delta.writes()[0].clone();
        match first.preimage {
            FileState::Absent => {
                let _ = fs::remove_file(first.path.target(&root).unwrap());
            }
            FileState::File { .. } => panic!("fixture first preimage must be absent"),
        }
        drop(txn);
        assert!(matches!(
            RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID),
            Err(RepositoryTxnError::CorruptPlan(error))
                if error.contains("impossible durable recovery layout")
        ));
    }

    {
        let (_temp, root, journals) = empty_test_repository();
        initialize_failpoint_repository(&root);
        let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
        let plan = fixture_plan(&root, &draft, b"out of order postimage");
        let operation_id = plan.operation_id.clone();
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        assert_injected(
            txn.install_at_failpoint(RepositoryTxnStep::AfterInstallingJournalWrite { index: 0 }),
            RepositoryTxnStep::AfterInstallingJournalWrite { index: 0 },
        );
        let later = txn.journal.plan.canonical_delta.writes()[2].clone();
        txn.install_write(&later).unwrap();
        drop(txn);
        assert!(matches!(
            RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID),
            Err(RepositoryTxnError::CorruptPlan(error))
                if error.contains("postimage") && error.contains("preimage hole")
        ));
        assert_eq!(
            inspect_file_state(&root, &later.path).unwrap(),
            later.postimage
        );
    }
}

#[test]
fn recovery_conflict_is_typed_and_never_overwrites_third_party_drift() {
    let (_temp, root, journals) = empty_test_repository();
    initialize_failpoint_repository(&root);
    let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
    let plan = fixture_plan(&root, &draft, b"later recovery conflict");
    let operation_id = plan.operation_id.clone();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    txn.mark_committed().unwrap();
    let conflicted = txn.journal.plan.canonical_delta.writes()[2].clone();
    let target = conflicted.path.target(&root).unwrap();
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"third-party drift").unwrap();
    drop(txn);

    assert!(matches!(
        RepositoryTxn::recover(&root, &journals, &operation_id, TEST_REPOSITORY_ID),
        Err(RepositoryTxnError::CommittedConflict { path, .. }) if path == conflicted.path
    ));
    assert_eq!(fs::read(&target).unwrap(), b"third-party drift");
    assert!(!root.join("records/evidence.json").exists());
}

#[test]
fn barrier_diagnostic_validates_the_full_set_before_emitting_an_exact_hint() {
    {
        let (_temp, root, journals) = empty_test_repository();
        let (_, paths) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/malformed-barrier.json",
            b"malformed barrier marker",
        );
        fs::create_dir_all(paths.marker.parent().unwrap()).unwrap();
        fs::write(&paths.marker, b"{").unwrap();
        assert!(matches!(
            RepositoryTxn::verify_recovery_barrier(&root, &journals),
            Err(RepositoryTxnError::Journal(_))
        ));
    }

    {
        let (_temp, root, journals) = empty_test_repository();
        let (_, paths) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/missing-blob-barrier.json",
            b"missing barrier blob",
        );
        let journal: RepositoryTxnJournal = operation_journal::read_json(&paths.plan).unwrap();
        let digest = &journal.plan.canonical_delta.writes()[0]
            .payload
            .as_ref()
            .unwrap()
            .digest;
        fs::remove_file(paths.blob(digest)).unwrap();
        assert!(matches!(
            RepositoryTxn::verify_recovery_barrier(&root, &journals),
            Err(RepositoryTxnError::MissingBlob(actual)) if actual == *digest
        ));
    }

    {
        let (_temp, root, journals) = empty_test_repository();
        let (first, first_paths) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/first-barrier.json",
            b"first barrier",
        );
        let hidden = _temp.path().join("first-barrier-hidden.json");
        fs::rename(&first_paths.plan, &hidden).unwrap();
        let (second, _) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/second-barrier.json",
            b"second barrier",
        );
        fs::rename(&hidden, &first_paths.plan).unwrap();
        let error = RepositoryTxn::verify_recovery_barrier(&root, &journals).unwrap_err();
        assert!(matches!(
            error,
            RepositoryTxnError::MultiplePendingTransactions { operation_ids }
                if operation_ids == {
                    let mut expected = vec![
                        first.as_str().to_string(),
                        second.as_str().to_string(),
                    ];
                    expected.sort();
                    expected
                }
        ));
    }
}

#[test]
fn barrier_diagnostic_holds_an_existing_lock_and_ignores_atomic_temp_residue() {
    let (_temp, root, journals) = empty_test_repository();
    let (draft, plan) = one_write_fixture(&root, "records/live-writer.json", b"live writer");
    let operation_id = plan.operation_id.clone();
    let txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();

    assert!(matches!(
        RepositoryTxn::verify_recovery_barrier(&root, &journals),
        Err(RepositoryTxnError::Busy)
    ));
    drop(txn);

    fs::write(
        journals.join("repository").join(TEST_OWNED_ATOMIC_TEMP),
        b"orphaned atomic temporary file",
    )
    .unwrap();
    assert!(matches!(
        RepositoryTxn::verify_recovery_barrier(&root, &journals),
        Err(RepositoryTxnError::RecoveryRequired {
            operation_id: blocked,
            state: RecoveryState::Prepared,
        }) if blocked == operation_id.as_str()
    ));
}

#[test]
fn completed_history_is_checked_before_terminal_or_incomplete_recovery_mutates() {
    let (_temp, root, journals) = empty_test_repository();
    let (completed_draft, completed_plan) = one_write_fixture(
        &root,
        "records/completed-history.json",
        b"completed history",
    );
    let completed_operation = completed_plan.operation_id.clone();
    let completed =
        RepositoryTxn::prepare(&root, &journals, completed_plan, completed_draft).unwrap();
    mark_install_complete(completed);

    let (pending_operation, pending_paths) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/pending-history.json",
        b"pending history",
    );
    fs::remove_file(root.join("records/completed-history.json")).unwrap();

    assert!(matches!(
        RepositoryTxn::recover(&root, &journals, &pending_operation, TEST_REPOSITORY_ID),
        Err(RepositoryTxnError::CompletedPostimageMismatch { .. })
    ));
    let pending: RepositoryTxnJournal = operation_journal::read_json(&pending_paths.plan).unwrap();
    assert_eq!(pending.recovery, RecoveryState::Prepared);
    assert!(!root.join("records/pending-history.json").exists());

    assert!(matches!(
        RepositoryTxn::recover(&root, &journals, &completed_operation, TEST_REPOSITORY_ID),
        Err(RepositoryTxnError::CompletedPostimageMismatch { .. })
    ));
}

#[test]
fn recovery_binds_the_caller_retained_repository_identity_under_lock() {
    let (_temp, root, journals) = empty_test_repository();
    let (operation_id, paths) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/identity-bound.json",
        b"identity-bound recovery",
    );
    let expected = "33333333-3333-4333-8333-333333333333";
    assert!(matches!(
        RepositoryTxn::recover(&root, &journals, &operation_id, "different-repository"),
        Err(RepositoryTxnError::RepositoryIdentityMismatch {
            expected: retained,
            actual,
        }) if retained == "different-repository" && actual == expected
    ));
    let journal: RepositoryTxnJournal = operation_journal::read_json(&paths.plan).unwrap();
    assert_eq!(journal.recovery, RecoveryState::Prepared);
    assert!(!root.join("records/identity-bound.json").exists());

    assert_eq!(
        RepositoryTxn::recover(&root, &journals, &operation_id, expected)
            .unwrap()
            .outcome,
        RecoveryOutcome::AbortedPrepared
    );
}

#[test]
fn recovery_inventory_rejects_mixed_repository_identities_before_mutation() {
    const CURRENT_ID: &str = "33333333-3333-4333-8333-333333333333";
    const FOREIGN_ID: &str = "44444444-4444-4444-8444-444444444444";

    {
        let (_temp, root, journals) = empty_test_repository();
        let (foreign_draft, foreign_plan) =
            one_write_fixture(&root, "records/foreign-terminal.json", b"foreign terminal");
        let foreign =
            RepositoryTxn::prepare(&root, &journals, foreign_plan, foreign_draft).unwrap();
        let foreign_paths = mark_install_complete(foreign);
        let mut foreign_journal: RepositoryTxnJournal =
            operation_journal::read_json(&foreign_paths.plan).unwrap();
        foreign_journal.plan.repository.repository_id = FOREIGN_ID.into();
        foreign_journal.plan.root = foreign_journal.plan.compute_root().unwrap();
        operation_journal::write_json(&foreign_paths.plan, &foreign_journal).unwrap();
        operation_journal::write_json(
            &foreign_paths.marker,
            &CommitMarker::from_plan(&foreign_journal.plan),
        )
        .unwrap();

        let (pending, pending_paths) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/current-pending.json",
            b"current pending",
        );
        assert!(matches!(
            RepositoryTxn::verify_recovery_barrier(&root, &journals),
            Err(RepositoryTxnError::MixedRepositoryIdentities { .. })
        ));
        assert!(matches!(
            RepositoryTxn::recover(&root, &journals, &pending, CURRENT_ID),
            Err(RepositoryTxnError::MixedRepositoryIdentities { repository_ids })
                if repository_ids == [CURRENT_ID, FOREIGN_ID]
        ));
        let pending_journal: RepositoryTxnJournal =
            operation_journal::read_json(&pending_paths.plan).unwrap();
        assert_eq!(pending_journal.recovery, RecoveryState::Prepared);
        assert!(!root.join("records/current-pending.json").exists());
    }

    {
        let (_temp, root, journals) = empty_test_repository();
        let (selected_draft, selected_plan) =
            one_write_fixture(&root, "records/current-terminal.json", b"current terminal");
        let selected_operation = selected_plan.operation_id.clone();
        let selected =
            RepositoryTxn::prepare(&root, &journals, selected_plan, selected_draft).unwrap();
        mark_install_complete(selected);

        let (_, foreign_paths) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/foreign-pending.json",
            b"foreign pending",
        );
        let mut foreign_journal: RepositoryTxnJournal =
            operation_journal::read_json(&foreign_paths.plan).unwrap();
        foreign_journal.plan.repository.repository_id = FOREIGN_ID.into();
        foreign_journal.plan.root = foreign_journal.plan.compute_root().unwrap();
        operation_journal::write_json(&foreign_paths.plan, &foreign_journal).unwrap();

        assert!(matches!(
            RepositoryTxn::recover(
                &root,
                &journals,
                &selected_operation,
                CURRENT_ID,
            ),
            Err(RepositoryTxnError::MixedRepositoryIdentities { repository_ids })
                if repository_ids == [CURRENT_ID, FOREIGN_ID]
        ));
        assert!(!root.join("records/foreign-pending.json").exists());
    }
}

#[test]
fn recovery_failpoint_seam_reuses_the_production_engine_and_normal_retry() {
    let (_temp, root, journals) = empty_test_repository();
    let (draft, plan) = one_write_fixture(
        &root,
        "records/recovery-interruption.json",
        b"recovery interruption",
    );
    let operation_id = plan.operation_id.clone();
    let repository_id = plan.repository.repository_id().to_string();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    txn.mark_committed().unwrap();
    drop(txn);

    assert!(matches!(
        RepositoryTxn::recover_at_failpoint(
            &root,
            &journals,
            &operation_id,
            &repository_id,
            RepositoryTxnStep::AfterInstalledJournalWrite,
        ),
        Err(RepositoryTxnError::InjectedFailure {
            step: RepositoryTxnStep::AfterInstalledJournalWrite,
        })
    ));
    let interrupted: RepositoryTxnJournal =
        operation_journal::read_json(&RepositoryTxnPaths::new(&journals, &operation_id).plan)
            .unwrap();
    assert_eq!(interrupted.recovery, RecoveryState::Installed);
    assert_eq!(
        fs::read(root.join("records/recovery-interruption.json")).unwrap(),
        b"authorized postimage"
    );

    assert_eq!(
        RepositoryTxn::recover(&root, &journals, &operation_id, &repository_id,)
            .unwrap()
            .outcome,
        RecoveryOutcome::Completed
    );
}

#[test]
fn completed_operation_proof_is_locked_exact_and_read_only() {
    let (_temp, root, journals) = empty_test_repository();
    let (draft, plan) =
        one_write_fixture(&root, "records/completed-proof.json", b"completed proof");
    let expected_delta = draft.delta.clone();
    let operation_id = plan.operation_id.clone();
    let repository_id = plan.repository.repository_id.clone();
    let kind = plan.kind.clone();
    let request_root = plan.request_root.clone();
    let fixed_time = plan.fixed_time.clone();
    let read_set = plan.read_set.clone();
    let result = plan.result.clone();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
    txn.mark_committed().unwrap();
    txn.install().unwrap();
    txn.complete().unwrap();
    let paths = txn.paths.clone();
    let referenced_blob = txn
        .plan()
        .canonical_delta
        .writes()
        .first()
        .and_then(|write| write.payload.as_ref())
        .map(|blob| paths.blob(&blob.digest))
        .unwrap();
    drop(txn);

    let stale_bytes = b"valid unreferenced recovery blob".to_vec();
    let stale_digest = ContentDigest::hash(&stale_bytes);
    let stale_blob = BlobJournal {
        schema: REPOSITORY_TXN_BLOB_SCHEMA.to_string(),
        digest: stale_digest.clone(),
        size: stale_bytes.len() as u64,
        bytes: stale_bytes,
    };
    let stale_blob_path = paths.blob(&stale_digest);
    operation_journal::write_json(&stale_blob_path, &stale_blob).unwrap();
    let repository_temp = journals.join("repository").join(TEST_OWNED_ATOMIC_TEMP);
    let blob_temp = paths.blob_dir.join(TEST_OWNED_ATOMIC_TEMP);
    let marker_temp = paths.marker.parent().unwrap().join(TEST_OWNED_ATOMIC_TEMP);
    for temp in [&repository_temp, &blob_temp, &marker_temp] {
        fs::write(temp, b"owned atomic-write residue").unwrap();
    }

    let expected = CompletedOperationExpectation {
        repository_id: &repository_id,
        kind: &kind,
        request_root: &request_root,
        fixed_time: &fixed_time,
        result: &result,
    };
    let before = snapshot_files(&journals);
    let verified =
        RepositoryTxn::verify_completed_operation(&root, &journals, &operation_id, &expected)
            .unwrap();
    assert_eq!(verified.canonical_delta(), &expected_delta);
    assert_eq!(verified.read_set(), read_set.as_slice());
    assert_ne!(verified.read_set(), &[]);
    let canonical_root = canonical_repository_root(&root).unwrap();
    let lock_path = RepositoryWriteLock::path(&journals, &canonical_root);
    let relative = |path: &Path| {
        path.strip_prefix(&journals)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    };
    let mut expected_residue = vec![
        (
            "repository".to_string(),
            ValidatedPrivateResidueKind::Directory,
        ),
        (
            "repository/blobs".to_string(),
            ValidatedPrivateResidueKind::Directory,
        ),
        (
            "repository/committed".to_string(),
            ValidatedPrivateResidueKind::Directory,
        ),
        (
            "repository-locks".to_string(),
            ValidatedPrivateResidueKind::Directory,
        ),
        (
            relative(&paths.plan),
            ValidatedPrivateResidueKind::RegularFile,
        ),
        (
            relative(&paths.marker),
            ValidatedPrivateResidueKind::RegularFile,
        ),
        (
            relative(&referenced_blob),
            ValidatedPrivateResidueKind::RegularFile,
        ),
        (
            relative(&stale_blob_path),
            ValidatedPrivateResidueKind::RegularFile,
        ),
        (
            relative(&repository_temp),
            ValidatedPrivateResidueKind::RegularFile,
        ),
        (
            relative(&blob_temp),
            ValidatedPrivateResidueKind::RegularFile,
        ),
        (
            relative(&marker_temp),
            ValidatedPrivateResidueKind::RegularFile,
        ),
        (
            relative(&lock_path),
            ValidatedPrivateResidueKind::RegularFile,
        ),
    ];
    expected_residue.sort();
    let observed_residue = verified
        .private_residue()
        .iter()
        .map(|entry| (entry.path().as_str().to_string(), entry.kind()))
        .collect::<Vec<_>>();
    assert_eq!(observed_residue, expected_residue);
    assert_eq!(snapshot_files(&journals), before);

    let mut malformed_stale_blob = stale_blob.clone();
    malformed_stale_blob.schema = "unowned.blob.schema".to_string();
    operation_journal::write_json(&stale_blob_path, &malformed_stale_blob).unwrap();
    assert!(matches!(
        RepositoryTxn::verify_completed_operation(
            &root,
            &journals,
            &operation_id,
            &expected,
        ),
        Err(RepositoryTxnError::CorruptBlob(digest)) if digest == stale_digest
    ));
    operation_journal::write_json(&stale_blob_path, &stale_blob).unwrap();
    assert_eq!(snapshot_files(&journals), before);

    let wrong_result = json!({"different": true});
    let mismatched = CompletedOperationExpectation {
        result: &wrong_result,
        ..expected
    };
    assert!(matches!(
        RepositoryTxn::verify_completed_operation(&root, &journals, &operation_id, &mismatched,),
        Err(RepositoryTxnError::CompletedOperationExpectationMismatch {
            field: "result",
            ..
        })
    ));

    fs::remove_file(&lock_path).unwrap();
    assert!(matches!(
        RepositoryTxn::verify_completed_operation(&root, &journals, &operation_id, &expected,),
        Err(RepositoryTxnError::RepositoryLockMissing)
    ));
    assert!(!lock_path.exists());
}

#[test]
fn named_terminal_recovery_reports_one_other_exact_blocker_idempotently() {
    for terminal_state in [RecoveryState::Completed, RecoveryState::Aborted] {
        let (_temp, root, journals) = empty_test_repository();

        let (selected_operation, selected_paths) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/selected.json",
            format!("selected {terminal_state:?}").as_bytes(),
        );
        match terminal_state {
            RecoveryState::Completed => {
                let mut selected =
                    RepositoryTxn::open(&root, &journals, &selected_operation).unwrap();
                selected.bind_exact_test_authorization().unwrap();
                selected.mark_committed().unwrap();
                selected.install().unwrap();
                selected.complete().unwrap();
            }
            RecoveryState::Aborted => {
                assert_eq!(
                    RepositoryTxn::recover(
                        &root,
                        &journals,
                        &selected_operation,
                        TEST_REPOSITORY_ID
                    )
                    .unwrap()
                    .outcome,
                    RecoveryOutcome::AbortedPrepared
                );
                assert!(!selected_paths.marker.exists());
            }
            _ => unreachable!(),
        }

        let (next_operation, _) = persist_marker_free_prepared_fixture(
            &root,
            &journals,
            "records/next.json",
            format!("next after {terminal_state:?}").as_bytes(),
        );
        let result =
            RepositoryTxn::recover(&root, &journals, &selected_operation, TEST_REPOSITORY_ID)
                .unwrap();
        assert_eq!(result.operation_id, selected_operation);
        assert_eq!(result.prior_state, terminal_state);
        assert_eq!(
            result.outcome,
            if matches!(terminal_state, RecoveryState::Completed) {
                RecoveryOutcome::AlreadyCompleted
            } else {
                RecoveryOutcome::AlreadyAborted
            }
        );
        assert_eq!(result.next_operation_id, Some(next_operation.clone()));
        assert!(!root.join("records/next.json").exists());

        assert_eq!(
            RepositoryTxn::recover(&root, &journals, &next_operation, TEST_REPOSITORY_ID)
                .unwrap()
                .outcome,
            RecoveryOutcome::AbortedPrepared
        );
        let retry =
            RepositoryTxn::recover(&root, &journals, &selected_operation, TEST_REPOSITORY_ID)
                .unwrap();
        assert_eq!(retry.next_operation_id, None);
    }
}

#[test]
fn named_nonterminal_recovery_refuses_any_other_incomplete_before_mutation() {
    let (_temp, root, journals) = empty_test_repository();

    let (selected_operation, selected_paths) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/selected.json",
        b"selected pending",
    );
    let hidden_selected = _temp.path().join("selected-journal.json");
    fs::rename(&selected_paths.plan, &hidden_selected).unwrap();
    let (other_operation, _) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/other.json",
        b"other pending",
    );
    fs::rename(&hidden_selected, &selected_paths.plan).unwrap();

    let error = RepositoryTxn::recover(&root, &journals, &selected_operation, TEST_REPOSITORY_ID)
        .unwrap_err();
    assert!(matches!(
        error,
        RepositoryTxnError::AmbiguousRecovery {
            requested_operation_id,
            other_operation_ids,
        } if requested_operation_id == selected_operation.as_str()
            && other_operation_ids == [other_operation.as_str()]
    ));
    assert_eq!(
        RepositoryTxn::open(&root, &journals, &selected_operation)
            .unwrap()
            .recovery_state(),
        &RecoveryState::Prepared
    );
    assert!(!root.join("records/selected.json").exists());
    assert!(!root.join("records/other.json").exists());
}

#[test]
fn named_terminal_recovery_rejects_more_than_one_other_incomplete() {
    let (_temp, root, journals) = empty_test_repository();

    let (terminal_operation, _) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/terminal.json",
        b"terminal operation",
    );
    let mut terminal = RepositoryTxn::open(&root, &journals, &terminal_operation).unwrap();
    terminal.bind_exact_test_authorization().unwrap();
    mark_install_complete(terminal);

    let (first_operation, first_paths) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/first-pending.json",
        b"first pending",
    );
    let hidden_first = _temp.path().join("first-pending-journal.json");
    fs::rename(&first_paths.plan, &hidden_first).unwrap();
    let (second_operation, _) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/second-pending.json",
        b"second pending",
    );
    fs::rename(&hidden_first, &first_paths.plan).unwrap();

    let mut expected = vec![
        first_operation.as_str().to_string(),
        second_operation.as_str().to_string(),
    ];
    expected.sort();
    let error = RepositoryTxn::recover(&root, &journals, &terminal_operation, TEST_REPOSITORY_ID)
        .unwrap_err();
    assert!(matches!(
        error,
        RepositoryTxnError::AmbiguousRecovery {
            requested_operation_id,
            other_operation_ids,
        } if requested_operation_id == terminal_operation.as_str()
            && other_operation_ids == expected
    ));
    assert!(!root.join("records/first-pending.json").exists());
    assert!(!root.join("records/second-pending.json").exists());
}

#[test]
fn production_recovery_fails_closed_for_not_found_malformed_marker_and_wrong_root() {
    let (_temp, root, journals) = empty_test_repository();
    let absent = OperationId::derive("submission", b"absent recovery");
    assert!(matches!(
        RepositoryTxn::recover(&root, &journals, &absent, TEST_REPOSITORY_ID),
        Err(RepositoryTxnError::OperationNotFound { operation_id })
            if operation_id == absent.as_str()
    ));

    let (malformed_operation, malformed_paths) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/malformed.json",
        b"malformed marker recovery",
    );
    fs::create_dir_all(malformed_paths.marker.parent().unwrap()).unwrap();
    fs::write(&malformed_paths.marker, b"{").unwrap();
    assert!(matches!(
        RepositoryTxn::recover(&root, &journals, &malformed_operation, TEST_REPOSITORY_ID),
        Err(RepositoryTxnError::Journal(_))
    ));
    assert!(!root.join("records/malformed.json").exists());

    fs::remove_file(&malformed_paths.marker).unwrap();
    assert_eq!(
        RepositoryTxn::recover(&root, &journals, &malformed_operation, TEST_REPOSITORY_ID)
            .unwrap()
            .outcome,
        RecoveryOutcome::AbortedPrepared
    );

    let (mismatched_operation, mismatched_paths) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/mismatched-marker.json",
        b"mismatched marker recovery",
    );
    let journal: RepositoryTxnJournal =
        operation_journal::read_json(&mismatched_paths.plan).unwrap();
    let mut marker = CommitMarker::from_plan(&journal.plan);
    marker.plan_root = ContentDigest::hash(b"different plan");
    operation_journal::write_json(&mismatched_paths.marker, &marker).unwrap();
    assert!(matches!(
        RepositoryTxn::recover(&root, &journals, &mismatched_operation, TEST_REPOSITORY_ID),
        Err(RepositoryTxnError::CorruptPlan(error))
            if error.contains("does not match its durable plan")
    ));
    assert!(!root.join("records/mismatched-marker.json").exists());
    fs::remove_file(&mismatched_paths.marker).unwrap();
    assert_eq!(
        RepositoryTxn::recover(&root, &journals, &mismatched_operation, TEST_REPOSITORY_ID)
            .unwrap()
            .outcome,
        RecoveryOutcome::AbortedPrepared
    );

    let (empty_id_operation, empty_id_paths) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/empty-repository-id.json",
        b"empty repository id recovery",
    );
    let mut empty_id_journal: RepositoryTxnJournal =
        operation_journal::read_json(&empty_id_paths.plan).unwrap();
    let repository_id = empty_id_journal.plan.repository.repository_id.clone();
    empty_id_journal.plan.repository.repository_id = " ".into();
    empty_id_journal.plan.root = empty_id_journal.plan.compute_root().unwrap();
    operation_journal::write_json(&empty_id_paths.plan, &empty_id_journal).unwrap();
    assert!(matches!(
        RepositoryTxn::recover(&root, &journals, &empty_id_operation, TEST_REPOSITORY_ID),
        Err(RepositoryTxnError::CorruptPlan(error))
            if error.contains("empty repository id")
    ));
    assert!(!root.join("records/empty-repository-id.json").exists());
    empty_id_journal.plan.repository.repository_id = repository_id;
    empty_id_journal.plan.root = empty_id_journal.plan.compute_root().unwrap();
    operation_journal::write_json(&empty_id_paths.plan, &empty_id_journal).unwrap();
    assert_eq!(
        RepositoryTxn::recover(&root, &journals, &empty_id_operation, TEST_REPOSITORY_ID)
            .unwrap()
            .outcome,
        RecoveryOutcome::AbortedPrepared
    );

    let (wrong_root_operation, wrong_root_paths) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/wrong-root.json",
        b"wrong root recovery",
    );
    let mut journal: RepositoryTxnJournal =
        operation_journal::read_json(&wrong_root_paths.plan).unwrap();
    journal.plan.repository.canonical_root = "/different/repository".into();
    journal.plan.root = journal.plan.compute_root().unwrap();
    operation_journal::write_json(&wrong_root_paths.plan, &journal).unwrap();
    assert!(matches!(
        RepositoryTxn::recover(&root, &journals, &wrong_root_operation, TEST_REPOSITORY_ID),
        Err(RepositoryTxnError::RepositoryBindingMismatch { .. })
    ));
    assert!(!root.join("records/wrong-root.json").exists());
}

#[test]
fn production_recovery_rejects_a_journal_stored_under_another_operation_id() {
    let (_temp, root, journals) = empty_test_repository();
    let (_, paths) = persist_marker_free_prepared_fixture(
        &root,
        &journals,
        "records/wrong-name.json",
        b"wrong journal name",
    );
    let wrong_name = OperationId::derive("submission", b"different journal name");
    let wrong_paths = RepositoryTxnPaths::new(&journals, &wrong_name);
    fs::rename(&paths.plan, &wrong_paths.plan).unwrap();

    assert!(matches!(
        RepositoryTxn::recover(&root, &journals, &wrong_name, TEST_REPOSITORY_ID),
        Err(RepositoryTxnError::CorruptPlan(error))
            if error.contains("stored under the wrong journal name")
    ));
    assert!(!root.join("records/wrong-name.json").exists());
}

#[test]
fn path_bound_file_snapshot_commits_supplied_bytes_without_rereading() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repository");
    fs::create_dir_all(root.join(".vela/policies")).unwrap();
    let policy_path = RepoPath::parse(".vela/policies/active.json").unwrap();

    let snapshot =
        InputBinding::file_snapshot(policy_path.clone(), Some(b"loaded policy")).unwrap();
    fs::write(root.join(policy_path.as_str()), b"loaded policy").unwrap();
    snapshot.verify_current(&root).unwrap();

    fs::write(root.join(policy_path.as_str()), b"rotated policy").unwrap();
    assert!(matches!(
        snapshot.verify_current(&root),
        Err(RepositoryTxnError::StaleInput { path, .. }) if path == policy_path
    ));

    let signature_path = RepoPath::parse(".vela/policies/active.sig.json").unwrap();
    let absent = InputBinding::file_snapshot(signature_path.clone(), None).unwrap();
    absent.verify_current(&root).unwrap();
    fs::write(root.join(signature_path.as_str()), b"new signature").unwrap();
    assert!(matches!(
        absent.verify_current(&root),
        Err(RepositoryTxnError::StaleInput { path, .. }) if path == signature_path
    ));
}

#[test]
fn path_bound_existing_input_drift_refuses_the_commit_marker() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repository");
    let journals = temp.path().join("journals");
    fs::create_dir_all(root.join(".vela/policies")).unwrap();
    let policy_path = RepoPath::parse(".vela/policies/active.json").unwrap();
    fs::write(root.join(policy_path.as_str()), b"policy before").unwrap();
    let policy_input = InputBinding::existing_file(&root, policy_path.clone()).unwrap();
    let draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("pending.json").unwrap(),
            WriteClass::PublicReview,
            b"pending".to_vec(),
        )],
    )
    .unwrap();
    let mut plan = fixture_plan(&root, &draft, b"policy input drift");
    plan.read_set.push(policy_input);
    plan.read_set
        .sort_by(|left, right| left.name.cmp(&right.name));
    plan.root = plan.compute_root().unwrap();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();

    fs::write(root.join(policy_path.as_str()), b"policy after").unwrap();
    let error = txn.mark_committed().unwrap_err();

    assert!(matches!(
        error,
        RepositoryTxnError::StaleInput { path, .. } if path == policy_path
    ));
    assert_eq!(txn.recovery_state(), &RecoveryState::Aborted);
    assert!(!txn.paths.marker.exists());
    assert!(!root.join("pending.json").exists());
    drop(txn);
    assert_eq!(
        RepositoryTxn::recover(
            &root,
            &journals,
            &OperationId::derive("submission", b"policy input drift"),
            TEST_REPOSITORY_ID,
        )
        .unwrap()
        .outcome,
        RecoveryOutcome::AlreadyAborted
    );

    // The aborted journal is terminal and does not block an unrelated
    // operation from planning and completing.
    let unrelated_draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("unrelated.json").unwrap(),
            WriteClass::CanonicalEvidence,
            b"unrelated".to_vec(),
        )],
    )
    .unwrap();
    let unrelated_plan = fixture_plan(&root, &unrelated_draft, b"unrelated after policy abort");
    let mut unrelated =
        RepositoryTxn::prepare(&root, &journals, unrelated_plan, unrelated_draft).unwrap();
    unrelated.mark_committed().unwrap();
    unrelated.install().unwrap();
    unrelated.complete().unwrap();
    drop(unrelated);

    // The same normalized request may also replan against the policy bytes
    // that are current now, replacing its marker-free aborted plan.
    let retry_draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("pending.json").unwrap(),
            WriteClass::PublicReview,
            b"pending".to_vec(),
        )],
    )
    .unwrap();
    let mut retry_plan = fixture_plan(&root, &retry_draft, b"policy input drift");
    retry_plan
        .read_set
        .push(InputBinding::existing_file(&root, policy_path.clone()).unwrap());
    retry_plan
        .read_set
        .sort_by(|left, right| left.name.cmp(&right.name));
    retry_plan.root = retry_plan.compute_root().unwrap();
    let mut retry = RepositoryTxn::prepare(&root, &journals, retry_plan, retry_draft).unwrap();
    retry.mark_committed().unwrap();
    retry.install().unwrap();
    retry.complete().unwrap();
    assert_eq!(retry.recovery_state(), &RecoveryState::Completed);
}

#[test]
fn path_bound_absent_input_creation_refuses_the_commit_marker() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repository");
    let journals = temp.path().join("journals");
    fs::create_dir_all(root.join(".vela/policies")).unwrap();
    let signature_path = RepoPath::parse(".vela/policies/active.sig.json").unwrap();
    let signature_input = InputBinding::absent_file(&root, signature_path.clone()).unwrap();
    let draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("pending.json").unwrap(),
            WriteClass::PublicReview,
            b"pending".to_vec(),
        )],
    )
    .unwrap();
    let mut plan = fixture_plan(&root, &draft, b"absent policy input drift");
    plan.read_set.push(signature_input);
    plan.read_set
        .sort_by(|left, right| left.name.cmp(&right.name));
    plan.root = plan.compute_root().unwrap();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();

    fs::write(root.join(signature_path.as_str()), b"new signature").unwrap();
    assert!(matches!(
        txn.mark_committed(),
        Err(RepositoryTxnError::StaleInput { path, .. }) if path == signature_path
    ));
    assert!(!txn.paths.marker.exists());
    assert!(!root.join("pending.json").exists());
}

#[cfg(unix)]
#[test]
fn path_bound_input_rejects_a_symlink_swap_before_the_marker() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repository");
    let journals = temp.path().join("journals");
    fs::create_dir_all(root.join(".vela/policies")).unwrap();
    let outside = temp.path().join("outside-policy.json");
    fs::write(&outside, b"outside").unwrap();
    let policy_path = RepoPath::parse(".vela/policies/active.json").unwrap();
    let policy_target = root.join(policy_path.as_str());
    fs::write(&policy_target, b"policy before").unwrap();
    let policy_input = InputBinding::existing_file(&root, policy_path).unwrap();
    let draft = DeltaDraft::prepare(
        &root,
        vec![PlannedWrite::write(
            RepoPath::parse("pending.json").unwrap(),
            WriteClass::PublicReview,
            b"pending".to_vec(),
        )],
    )
    .unwrap();
    let mut plan = fixture_plan(&root, &draft, b"policy symlink swap");
    plan.read_set.push(policy_input);
    plan.read_set
        .sort_by(|left, right| left.name.cmp(&right.name));
    plan.root = plan.compute_root().unwrap();
    let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();

    fs::remove_file(&policy_target).unwrap();
    symlink(&outside, &policy_target).unwrap();
    assert!(matches!(
        txn.mark_committed(),
        Err(RepositoryTxnError::UnsafeTarget { .. })
    ));
    assert!(!txn.paths.marker.exists());
}
