//! `vela check` payload assembly and hashing plumbing: schema/lint/replay
//! diagnostics, repair-plan derivation, sensitive-path scanning, and the
//! frontier hash helpers. Moved verbatim from `cli/mod.rs`.

use super::*;

#[allow(clippy::too_many_arguments)]
/// v0.113: walk a frontier path and return any files whose names
/// match shapes commonly associated with secrets: literal extensions
/// (`*.key`, `*.pem`, `*.p12`) and substring patterns (`private`,
/// `secret`, `credential`). Skips standard noise (`.git/`, `target/`,
/// `node_modules/`, `dist/`, `build/`). Used by `vela check --strict`
/// and by `scripts/test-secret-audit.sh`. Closes part of
/// THREAT_MODEL.md A17 with active detection on top of the passive
/// .gitignore exclusion shipped at v0.111.1.
pub fn scan_for_sensitive_paths(root: &Path) -> Vec<PathBuf> {
    let mut hits: Vec<PathBuf> = Vec::new();
    let skip_dirs: &[&str] = &[".git", "target", "node_modules", "dist", "build"];
    let bad_exts: &[&str] = &["key", "pem", "p12", "pfx"];
    let bad_substrings: &[&str] = &["private", "secret", "credential"];
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name_os = path.file_name();
            let Some(name) = name_os.and_then(|n| n.to_str()) else {
                continue;
            };
            let lower = name.to_lowercase();
            if path.is_dir() {
                if skip_dirs.contains(&name) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            // .pub and .pubkey files are public-key material; skip.
            if lower.ends_with(".pub") || lower.ends_with(".pubkey") {
                continue;
            }
            // public.key by name is an Ed25519 PUBLIC key; safe.
            if lower == "public.key" {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_lowercase)
                .unwrap_or_default();
            let mut hit = false;
            if bad_exts.iter().any(|x| ext == *x) {
                hit = true;
            }
            if bad_substrings.iter().any(|s| lower.contains(s)) {
                hit = true;
            }
            if hit {
                hits.push(path);
            }
        }
    }
    hits.sort();
    hits
}

pub(crate) fn frontier_dir_for_source(source: &Path) -> &Path {
    if source.is_dir() {
        source
    } else {
        source
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
    }
}

pub(crate) fn preserves_anchored_legacy_proposal_parity(
    repository_context: &Value,
    parity_conflict_count: usize,
) -> bool {
    parity_conflict_count > 0
        && repository_context.get("valid").and_then(Value::as_bool) == Some(true)
        && repository_context.get("generation").and_then(Value::as_str) == Some("profile_v1")
        && repository_context
            .get("identity_mode")
            .and_then(Value::as_str)
            == Some("pinned_boundary")
}

pub(crate) fn proposal_parity_blocks(
    repository_context: &Value,
    parity_conflict_count: usize,
) -> bool {
    parity_conflict_count > 0
        && !preserves_anchored_legacy_proposal_parity(repository_context, parity_conflict_count)
}

pub(crate) fn proposal_parity_summary(
    repository_context: &Value,
    parity_conflict_count: usize,
) -> String {
    if parity_conflict_count == 0 {
        "ok".to_string()
    } else if preserves_anchored_legacy_proposal_parity(repository_context, parity_conflict_count) {
        format!(
            "{parity_conflict_count} anchored immutable legacy identity record(s) (unauthenticated)"
        )
    } else {
        format!("{parity_conflict_count} conflict(s)")
    }
}

fn proposal_parity_suggestion(repository_context: &Value, conflict: &str) -> &'static str {
    if repository_context.get("generation").and_then(Value::as_str) == Some("legacy_v0_1")
        && conflict.contains("logical content derives id")
    {
        "Do not rewrite or re-issue this immutable proposal. Inspect the predecessor repository with its pinned historical Vela release; the current binary does not convert predecessor repositories."
    } else {
        "Every decided proposal must have a signed review.* event (or, for accepts, its domain event). Re-issue the exact action through `vela review accept` or `vela review reject`."
    }
}

pub(crate) fn active_policy_pair_snapshot(
    source: &Path,
) -> Result<vela_protocol::acceptance_policy::ActivePolicySnapshot, String> {
    vela_protocol::acceptance_policy::load_active_policy_snapshot(frontier_dir_for_source(source))
}

const REPOSITORY_CONTEXT_CHECK_ID: &str = "repository_context";

pub(crate) struct RepositoryContextAssessment {
    pub(crate) payload: Value,
    /// Independently retained consumer pin, converted to the narrow form
    /// accepted by read-only Target Index verification. This is populated
    /// only after the complete repository context has verified successfully;
    /// repository-controlled bytes can never manufacture it.
    #[allow(dead_code)]
    pub(crate) target_index_trust_anchor:
        Option<vela_edge::frontier_repository::RepositoryTrustAnchor>,
    /// Repository Authority v2 events returned only after the complete
    /// authority history and repository context verify. Read projections may
    /// use these events for proposal parity, never as an authority grant.
    #[allow(dead_code)]
    pub(crate) authority_events: Vec<vela_protocol::authority::AuthorityEventV1>,
}

fn repository_context_not_applicable() -> Value {
    json!({
        "id": REPOSITORY_CONTEXT_CHECK_ID,
        "status": "not_applicable",
        "valid": null,
        "generation": null,
        "checked": 0,
        "failed": 0,
        "code": null,
        "error": null,
        "trust_anchor_root": null,
    })
}

fn repository_context_legacy(generation: &str) -> Value {
    json!({
        "id": REPOSITORY_CONTEXT_CHECK_ID,
        "status": "pass",
        "valid": true,
        "generation": generation,
        "checked": 1,
        "failed": 0,
        "code": null,
        "error": null,
        "trust_anchor_root": null,
        "compatibility": "read_only_replay",
    })
}

fn repository_context_invalid(code: &str, message: impl Into<String>) -> Value {
    json!({
        "id": REPOSITORY_CONTEXT_CHECK_ID,
        "status": "fail",
        "valid": false,
        "generation": "profile_v1",
        "checked": 1,
        "failed": 1,
        "code": code,
        "error": message.into(),
        "trust_anchor_root": null,
    })
}

fn repository_context_suggestion(code: &str) -> &'static str {
    match code {
        "repository_trust_anchor_required" => {
            "Inspect the exact first administrator boundary, then install its independently reviewed consumer pin with `vela frontier trust pin`; never derive trust from repository bytes alone."
        }
        "repository_trust_anchor_invalid" => {
            "Restore the exact independently reviewed consumer pin or stop using this checkout; an invalid or mismatched pin grants no repository-boundary validity."
        }
        "repository_boundary_invalid" => {
            "Restore complete Git ancestry and the exact anchored repository bytes. Missing, forked, non-ancestor, or root-mismatched boundaries grant no exemption."
        }
        "frontier_profile_upgrade_required" => {
            "Restore the exact current repository checkout. Use the pinned historical Vela release only to inspect a predecessor repository; the current binary does not convert it."
        }
        _ => {
            "Restore the exact Profile v1 repository, canonical event projection, and independently pinned boundary context before relying on repository identity."
        }
    }
}

/// Inspect the repository-generation and boundary context used by read-side
/// verification.
///
/// This function is deliberately read-only. Profile v0.1 remains replayable.
/// Profile v1 reuses the canonical write gate's complete profile, settings,
/// replay, actor/retained-byte, Git-anchor, and external trust-pin validation,
/// but does not acquire a transaction barrier or create a journal.
pub(crate) fn repository_context_assessment_with_home(
    source: &Path,
    trusted_home: Option<&Path>,
) -> RepositoryContextAssessment {
    repository_context_assessment_with_project_and_home(source, None, trusted_home)
}

/// Verify repository context while reusing an exact project that the caller
/// has already loaded.
///
/// The repository write verifier still checks Git ancestry, retained bytes,
/// signatures, roots, and the independently stored trust anchor. This only
/// removes a redundant parse/signature pass from compound read projections.
pub(crate) fn repository_context_assessment_with_project_and_home(
    source: &Path,
    preloaded: Option<&project::Project>,
    trusted_home: Option<&Path>,
) -> RepositoryContextAssessment {
    use vela_edge::frontier_repository::RepositoryTrustAnchor;
    use vela_edge::repository_write::{
        RepositoryWriteGateCode, VerifiedRepositoryIdentity, load_authority_trust_anchor_from_home,
        load_repository_trust_anchor_from_home, verify_repository_for_write_with_authority_events,
    };
    use vela_protocol::events::EVENT_KIND_FRONTIER_REPOSITORY_BOUND;
    use vela_protocol::frontier_repo::{FrontierProfileFile, read_repository_profile};

    let repository = frontier_dir_for_source(source);
    if !repository.join(".vela").is_dir() && !repository.join("frontier.yaml").exists() {
        return RepositoryContextAssessment {
            payload: repository_context_not_applicable(),
            target_index_trust_anchor: None,
            authority_events: Vec::new(),
        };
    }

    let profile = match read_repository_profile(repository) {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            return RepositoryContextAssessment {
                payload: repository_context_invalid(
                    RepositoryWriteGateCode::FrontierProfileInvalid.as_str(),
                    "frontier.yaml is missing; repository generation and identity context are unknown",
                ),
                target_index_trust_anchor: None,
                authority_events: Vec::new(),
            };
        }
        Err(error) => {
            return RepositoryContextAssessment {
                payload: repository_context_invalid(
                    RepositoryWriteGateCode::FrontierProfileInvalid.as_str(),
                    error,
                ),
                target_index_trust_anchor: None,
                authority_events: Vec::new(),
            };
        }
    };

    match profile {
        FrontierProfileFile::LegacyV0_1(_) => RepositoryContextAssessment {
            payload: repository_context_legacy("legacy_v0_1"),
            target_index_trust_anchor: None,
            authority_events: Vec::new(),
        },
        FrontierProfileFile::V1(_) => {
            let loaded_project;
            let project = match preloaded {
                Some(project) => project,
                None => {
                    loaded_project = match repo::load_from_path(source) {
                        Ok(project) => project,
                        Err(error) => {
                            return RepositoryContextAssessment {
                                payload: repository_context_invalid(
                                    RepositoryWriteGateCode::RepositoryIdentityInvalid.as_str(),
                                    format!(
                                        "load Profile v1 repository for context verification: {error}"
                                    ),
                                ),
                                target_index_trust_anchor: None,
                                authority_events: Vec::new(),
                            };
                        }
                    };
                    &loaded_project
                }
            };
            let has_administrator_boundary = project
                .events
                .iter()
                .any(|event| event.kind.as_str() == EVENT_KIND_FRONTIER_REPOSITORY_BOUND);
            let loaded_anchor = if has_administrator_boundary {
                let home = match trusted_home {
                    Some(home) => std::fs::canonicalize(home).map_err(|error| {
                        format!("resolve operating-system account home for trust store: {error}")
                    }),
                    None => crate::frontier_txn::operating_system_account_home()
                        .map_err(|error| error.to_string())
                        .and_then(|home| {
                            std::fs::canonicalize(&home).map_err(|error| {
                                format!(
                                    "resolve operating-system account home for trust store: {error}"
                                )
                            })
                        }),
                };
                let home = match home {
                    Ok(home) => home,
                    Err(error) => {
                        return RepositoryContextAssessment {
                            payload: repository_context_invalid(
                                RepositoryWriteGateCode::RepositoryTrustAnchorInvalid.as_str(),
                                error,
                            ),
                            target_index_trust_anchor: None,
                            authority_events: Vec::new(),
                        };
                    }
                };
                match load_repository_trust_anchor_from_home(&home, &project.frontier_id()) {
                    Ok(anchor) => anchor,
                    Err(error) => {
                        return RepositoryContextAssessment {
                            payload: repository_context_invalid(
                                RepositoryWriteGateCode::RepositoryTrustAnchorInvalid.as_str(),
                                error,
                            ),
                            target_index_trust_anchor: None,
                            authority_events: Vec::new(),
                        };
                    }
                }
            } else {
                None
            };
            let authority = match crate::cli::load_repository_authority(repository, project) {
                Ok(authority) => authority,
                Err(error) => {
                    return RepositoryContextAssessment {
                        payload: repository_context_invalid(
                            RepositoryWriteGateCode::RepositoryBoundaryInvalid.as_str(),
                            format!("verify repository-authority history for context: {error}"),
                        ),
                        target_index_trust_anchor: None,
                        authority_events: Vec::new(),
                    };
                }
            };
            let authority_trust_anchor_root = if let Some(authority) = authority.as_ref() {
                let Some(first_root) = authority
                    .verification
                    .first_authority_record_root
                    .as_deref()
                else {
                    return RepositoryContextAssessment {
                        payload: repository_context_invalid(
                            RepositoryWriteGateCode::AuthorityTrustAnchorInvalid.as_str(),
                            "verified repository-authority history has no sequence-1 root",
                        ),
                        target_index_trust_anchor: None,
                        authority_events: Vec::new(),
                    };
                };
                let home = match trusted_home {
                    Some(home) => std::fs::canonicalize(home).map_err(|error| {
                        format!("resolve operating-system account home for trust store: {error}")
                    }),
                    None => crate::frontier_txn::operating_system_account_home()
                        .map_err(|error| error.to_string())
                        .and_then(|home| {
                            std::fs::canonicalize(&home).map_err(|error| {
                                format!(
                                    "resolve operating-system account home for trust store: {error}"
                                )
                            })
                        }),
                };
                let home = match home {
                    Ok(home) => home,
                    Err(error) => {
                        return RepositoryContextAssessment {
                            payload: repository_context_invalid(
                                RepositoryWriteGateCode::AuthorityTrustAnchorInvalid.as_str(),
                                error,
                            ),
                            target_index_trust_anchor: None,
                            authority_events: Vec::new(),
                        };
                    }
                };
                let loaded = match load_authority_trust_anchor_from_home(
                    &home,
                    &project.frontier_id(),
                ) {
                    Ok(Some(anchor)) => anchor,
                    Ok(None) => {
                        return RepositoryContextAssessment {
                            payload: repository_context_invalid(
                                RepositoryWriteGateCode::AuthorityTrustAnchorRequired.as_str(),
                                format!(
                                    "repository authority requires an independent sequence-1 pin; obtain {first_root} through a trusted channel, then run `vela authority trust pin . --record-root {first_root} --json`"
                                ),
                            ),
                            target_index_trust_anchor: None,
                            authority_events: Vec::new(),
                        };
                    }
                    Err(error) => {
                        return RepositoryContextAssessment {
                            payload: repository_context_invalid(
                                RepositoryWriteGateCode::AuthorityTrustAnchorInvalid.as_str(),
                                error,
                            ),
                            target_index_trust_anchor: None,
                            authority_events: Vec::new(),
                        };
                    }
                };
                if let Err(error) = loaded
                    .anchor
                    .verify_sequence_one(&project.frontier_id(), first_root)
                {
                    return RepositoryContextAssessment {
                        payload: repository_context_invalid(
                            RepositoryWriteGateCode::AuthorityTrustAnchorInvalid.as_str(),
                            error,
                        ),
                        target_index_trust_anchor: None,
                        authority_events: Vec::new(),
                    };
                }
                Some(loaded.root)
            } else {
                None
            };
            let authority_events = authority
                .map(|authority| authority.history.authority_events)
                .unwrap_or_default();
            match verify_repository_for_write_with_authority_events(
                repository,
                project,
                loaded_anchor.as_ref().map(|loaded| &loaded.anchor),
                &authority_events,
            ) {
                Ok(context) => {
                    let target_index_trust_anchor =
                        loaded_anchor.as_ref().map(|loaded| RepositoryTrustAnchor {
                            boundary_content_root: loaded.anchor.boundary_content_root.clone(),
                            administrator_public_key: loaded
                                .anchor
                                .administrator_public_key
                                .clone(),
                        });
                    let (identity_mode, trust_anchor_root) = match context.identity {
                        VerifiedRepositoryIdentity::Genesis { .. } => ("genesis", None),
                        VerifiedRepositoryIdentity::PinnedBoundary {
                            trust_anchor_root, ..
                        } => ("pinned_boundary", Some(trust_anchor_root)),
                    };
                    RepositoryContextAssessment {
                        payload: json!({
                            "id": REPOSITORY_CONTEXT_CHECK_ID,
                            "status": "pass",
                            "valid": true,
                            "generation": "profile_v1",
                            "checked": 1,
                            "failed": 0,
                            "code": null,
                            "error": null,
                            "frontier_id": context.frontier_id,
                            "profile_root": context.profile.profile_root,
                            "identity_root": context.profile.identity_root,
                            "identity_event_root": context.profile.identity_event_root,
                            "scientific_state_root": context.profile.scientific_state_root,
                            "identity_mode": identity_mode,
                            "trust_anchor_root": trust_anchor_root,
                            "authority_trust_anchor_root": authority_trust_anchor_root,
                        }),
                        target_index_trust_anchor,
                        authority_events,
                    }
                }
                Err(error) => RepositoryContextAssessment {
                    payload: repository_context_invalid(error.code.as_str(), error.message),
                    target_index_trust_anchor: None,
                    authority_events: Vec::new(),
                },
            }
        }
    }
}

fn repository_context_check_with_home(source: &Path, trusted_home: Option<&Path>) -> Value {
    repository_context_assessment_with_home(source, trusted_home).payload
}

pub(crate) fn repository_context_check(source: &Path) -> Value {
    repository_context_check_with_home(source, None)
}

pub(crate) fn check_json_payload(src: &Path, schema_only: bool, strict: bool) -> Value {
    check_json_payload_with_home(src, schema_only, strict, None)
}

pub(crate) fn check_json_payload_with_home(
    src: &Path,
    schema_only: bool,
    strict: bool,
    trusted_home: Option<&Path>,
) -> Value {
    let loaded = repo::load_from_path(src).ok();
    let repository_context = if schema_only {
        repository_context_not_applicable()
    } else {
        repository_context_assessment_with_project_and_home(src, loaded.as_ref(), trusted_home)
            .payload
    };
    check_json_payload_with_preloaded(
        src,
        schema_only,
        strict,
        loaded.as_ref(),
        repository_context,
    )
}

/// Assemble the canonical check payload from one already verified project and
/// repository-context assessment.
///
/// This is an internal composition edge for read projections. It preserves the
/// complete `check --strict` contract; callers may not synthesize the context
/// and production entry points still derive it through the repository verifier.
pub(crate) fn check_json_payload_with_preloaded(
    src: &Path,
    schema_only: bool,
    strict: bool,
    loaded: Option<&project::Project>,
    mut repository_context: Value,
) -> Value {
    let authority_events = loaded
        .and_then(|frontier| {
            crate::cli::load_repository_authority(frontier_dir_for_source(src), frontier)
                .ok()
                .flatten()
        })
        .map(|authority| authority.history.authority_events)
        .unwrap_or_default();
    let effective_project = loaded.and_then(|frontier| {
        let events =
            vela_protocol::reducer::semantic_event_union(frontier, &authority_events).ok()?;
        let encoded = serde_json::to_value(frontier).ok()?;
        let mut effective: project::Project = serde_json::from_value(encoded).ok()?;
        effective.events = events;
        project::recompute_stats(&mut effective);
        Some(effective)
    });
    let checked_project = effective_project.as_ref().or(loaded);
    let report = checked_project.map_or_else(
        || validate::validate(src),
        |frontier| validate::validate_loaded(src, frontier),
    );
    let (method_report, graph_report) = if schema_only {
        (None, None)
    } else if let Some(frontier) = checked_project {
        (
            Some(lint::lint(frontier, None, None)),
            Some(lint::lint_frontier(frontier)),
        )
    } else {
        (None, None)
    };
    // Once the Frontier has loaded, bind the check to the canonical project
    // bytes that the validators below actually inspect. Recursively hashing
    // the checkout made an otherwise read-only status depend on `.git`,
    // virtual environments, caches, and ignored recovery state. Besides being
    // needlessly expensive on real Frontiers, that broad hash could change
    // while every Vela-canonical byte remained identical.
    //
    // Keep the filesystem fallback for malformed inputs that cannot be loaded:
    // their diagnostic payload still needs a best-effort source identity.
    let source_hash = checked_project
        .and_then(canonical_project_hash)
        .unwrap_or_else(|| hash_path(src).unwrap_or_else(|_| "unavailable".to_string()));
    let mut diagnostics = Vec::new();
    diagnostics.extend(report.errors.iter().map(|e| {
        json!({
            "severity": "error",
            "rule_id": "schema",
            "finding_id": null,
            "file": &e.file,
            "field_path": null,
            "message": &e.error,
            "suggestion": schema_error_suggestion(&e.error),
            "fixable": schema_error_fix(&e.error),
            "normalize_action": schema_error_action(&e.error),
        })
    }));
    for (check_id, lint_report) in [
        ("methodology", method_report.as_ref()),
        ("frontier_graph", graph_report.as_ref()),
    ] {
        if let Some(lint_report) = lint_report {
            diagnostics.extend(lint_report.diagnostics.iter().map(|d| {
                json!({
                    "severity": d.severity.to_string(),
                    "rule_id": &d.rule_id,
                    "check": check_id,
                    "finding_id": &d.finding_id,
                    "field_path": null,
                    "message": &d.message,
                    "suggestion": &d.suggestion,
                    "fixable": false,
                    "normalize_action": null,
                })
            }));
        }
    }
    let method_errors = method_report.as_ref().map_or(0, |r| r.errors);
    let method_warnings = method_report.as_ref().map_or(0, |r| r.warnings);
    let method_infos = method_report.as_ref().map_or(0, |r| r.infos);
    let graph_errors = graph_report.as_ref().map_or(0, |r| r.errors);
    let graph_warnings = graph_report.as_ref().map_or(0, |r| r.warnings);
    let graph_infos = graph_report.as_ref().map_or(0, |r| r.infos);
    let replay_report = checked_project.map(events::replay_report);
    let state_integrity_report = checked_project.map(|frontier| {
        if schema_only {
            state_integrity::analyze(frontier)
        } else {
            state_integrity::analyze_loaded_path(src, frontier)
        }
    });
    if let Some(replay) = replay_report.as_ref()
        && !replay.ok
    {
        diagnostics.extend(replay.conflicts.iter().map(|conflict| {
            json!({
                "severity": "error",
                "rule_id": "event_replay",
                "check": "events",
                "finding_id": null,
                "field_path": null,
                "message": conflict,
                "suggestion": "Inspect canonical state events and repair the frontier event log before proof export.",
                "fixable": false,
                "normalize_action": null,
            })
        }));
    }
    // Review-decision parity: a stored proposal status with no signed,
    // replayable decision event behind it is a tamper-evidence failure.
    let parity_conflicts: Vec<String> = loaded
        .map(|frontier| {
            vela_protocol::proposals::verify_proposal_decision_parity_with_authority(
                frontier,
                &authority_events,
            )
        })
        .unwrap_or_default();
    // A verified pinned boundary may retain proposal IDs created under an
    // older logical-ID preimage. The repository write gate has already proved
    // that every such conflict existed at the exact Git anchor, that the
    // conflicted proposal bytes remain unchanged, and that no new conflict was
    // introduced. Report that historical debt explicitly, but do not run the
    // context-free parity rule a second time and misclassify it as current
    // tampering. Native Profile v1, legacy v0.1, and every invalid boundary
    // retain the ordinary blocking behavior.
    let preserved_legacy_parity =
        preserves_anchored_legacy_proposal_parity(&repository_context, parity_conflicts.len());
    if preserved_legacy_parity {
        repository_context["legacy_proposal_identity_debt"] = json!({
            "classification": "anchored_immutable_unauthenticated",
            "count": parity_conflicts.len(),
        });
    }
    if !parity_conflicts.is_empty() {
        diagnostics.extend(parity_conflicts.iter().map(|conflict| {
            if preserved_legacy_parity {
                json!({
                    "severity": "info",
                    "rule_id": "anchored_legacy_proposal_identity",
                    "check": "proposals",
                    "finding_id": null,
                    "field_path": ".vela/proposals",
                    "message": conflict,
                    "suggestion": "Retain the exact anchored proposal bytes. The historical ID is readable but unauthenticated by the current logical-ID rule; changing it or adding another conflict fails closed.",
                    "fixable": false,
                    "normalize_action": null,
                })
            } else {
                json!({
                    "severity": "error",
                    "rule_id": "review_decision_parity",
                    "check": "proposals",
                    "finding_id": null,
                    "field_path": null,
                    "message": conflict,
                    "suggestion": proposal_parity_suggestion(&repository_context, conflict),
                    "fixable": false,
                    "normalize_action": null,
                })
            }
        }));
    }
    let withdrawal_conflicts: Vec<String> = loaded
        .as_ref()
        .map(|frontier| {
            vela_protocol::proposals::verify_proposal_withdrawals(
                frontier_dir_for_source(src),
                frontier,
            )
        })
        .unwrap_or_default();
    diagnostics.extend(withdrawal_conflicts.iter().map(|conflict| {
        json!({
            "severity": "error",
            "rule_id": "invalid_proposal_withdrawal",
            "check": "proposals",
            "finding_id": null,
            "field_path": null,
            "message": conflict,
            "suggestion": "Treat the proposal as pending and restore the exact Receipt-bound signed withdrawal bytes.",
            "fixable": false,
            "normalize_action": null,
        })
    }));
    if repository_context.get("valid").and_then(Value::as_bool) == Some(false) {
        let code = repository_context
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("repository_boundary_invalid");
        let message = repository_context
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Profile v1 repository context did not validate");
        diagnostics.push(json!({
            "severity": "error",
            "rule_id": code,
            "check": REPOSITORY_CONTEXT_CHECK_ID,
            "finding_id": null,
            "field_path": "frontier.yaml|events[frontier.repository_bound]|git|<os-account-home>/.vela/trust/frontiers",
            "message": message,
            "suggestion": repository_context_suggestion(code),
            "fixable": false,
            "normalize_action": null,
        }));
    }
    // Active-pair integrity and current Permit readiness are assessed once.
    // Historical admissions remain the separate strict `policy_lane` check.
    let frontier_dir = frontier_dir_for_source(src);
    let active_policy_result = if !schema_only && loaded.is_some() {
        Some(active_policy_pair_snapshot(src))
    } else {
        None
    };
    let observed_at = chrono::Utc::now().to_rfc3339();
    let active_policy_assessment =
        active_policy_result
            .as_ref()
            .zip(loaded)
            .map(|(result, frontier)| {
                vela_protocol::proposals::policy_accept::assess_policy_readiness(
                    frontier,
                    result.as_ref().map_err(String::as_str),
                    &observed_at,
                )
            });
    let active_policy_errors = active_policy_result
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .cloned()
        .into_iter()
        .collect::<Vec<_>>();
    let policy_readiness_errors = active_policy_assessment
        .as_ref()
        .filter(|assessment| {
            assessment.state() != vela_protocol::proposals::policy_accept::PolicyState::Broken
                && assessment.permit_readiness()
                    == vela_protocol::proposals::policy_accept::PermitReadiness::Blocked
        })
        .map(|assessment| {
            assessment
                .detail()
                .unwrap_or("policy readiness assessment is blocked")
                .to_string()
        })
        .into_iter()
        .collect::<Vec<_>>();
    diagnostics.extend(active_policy_errors.iter().map(|error| {
        json!({
            "severity": "error",
            "rule_id": "active_policy_integrity",
            "check": "active_policy",
            "finding_id": null,
            "field_path": ".vela/policies/{active.json,active.sig.json}",
            "message": error,
            "suggestion": "Restore the exact active-policy pair or retire legacy bytes through the signed human governance ceremony; invalid policy bytes never fail open.",
            "fixable": false,
            "normalize_action": null,
        })
    }));
    diagnostics.extend(policy_readiness_errors.iter().map(|error| {
        json!({
            "severity": "error",
            "rule_id": "policy_head_integrity",
            "check": "policy_readiness",
            "finding_id": null,
            "field_path": "events[governance.policy_head]",
            "message": error,
            "suggestion": "Repair the signed policy-head chain through the human governance ceremony; malformed authority history never fails open.",
            "fixable": false,
            "normalize_action": null,
        })
    }));
    let policy_lane_errors = if strict && !schema_only {
        loaded
            .as_ref()
            .map(|frontier| {
                vela_protocol::proposals::policy_accept::verify_policy_lane_events(
                    frontier,
                    frontier_dir,
                )
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    diagnostics.extend(policy_lane_errors.iter().map(|conflict| {
        json!({
            "severity": "error",
            "rule_id": "policy_lane_replay",
            "check": "policy_lane",
            "finding_id": null,
            "field_path": "events[].payload.policy_lane",
            "message": conflict,
            "suggestion": "Restore the exact retained receipt, review material, signed policy snapshot, and causal policy-lane event; do not hand-edit accepted state.",
            "fixable": false,
            "normalize_action": null,
        })
    }));
    // Activity/state boundary: an activity-plane id (vac_/vrr_) in a
    // lineage-bearing position of accepted state is a soundness break (activity
    // is non-authoritative). Counted as a hard error, strict or not.
    let activity_leaks: Vec<(String, String)> = loaded
        .as_ref()
        .map(|f| {
            vela_protocol::activity::activity_ids_in_lineage(&f.findings, &f.verifier_attachments)
        })
        .unwrap_or_default();
    diagnostics.extend(activity_leaks.iter().map(|(holder, atom)| {
        json!({
            "severity": "error",
            "rule_id": "activity_state_boundary",
            "check": "lineage",
            "finding_id": holder,
            "field_path": null,
            "message": format!(
                "{holder} references activity-plane id {atom} in a lineage-bearing position; activity is non-authoritative and cannot enter accepted lineage"
            ),
            "suggestion": "Remove the activity id from the finding link / verifier attachment; reference the trace by content address in the activity plane instead.",
            "fixable": false,
            "normalize_action": null,
        })
    }));
    let activity_leak_errors = activity_leaks.len();
    let repository_context_errors =
        usize::from(repository_context.get("valid").and_then(Value::as_bool) == Some(false));
    let event_errors = replay_report
        .as_ref()
        .map_or(0, |replay| usize::from(!replay.ok))
        + usize::from(proposal_parity_blocks(
            &repository_context,
            parity_conflicts.len(),
        ))
        + policy_lane_errors.len();
    let state_integrity_errors = state_integrity_report
        .as_ref()
        .map_or(0, |report| report.structural_errors.len());
    let (source_registry, evidence_atoms, conditions, proposal_summary, proof_state) = loaded
        .as_ref()
        .map(|frontier| {
            (
                sources::source_summary(frontier),
                sources::evidence_summary(frontier),
                sources::condition_summary(frontier),
                proposals::summary(frontier),
                proposals::proof_state_json(&frontier.proof_state),
            )
        })
        .unwrap_or_else(|| {
            (
                sources::SourceRegistrySummary::default(),
                sources::EvidenceAtomSummary::default(),
                sources::ConditionSummary::default(),
                proposals::ProposalSummary::default(),
                Value::Null,
            )
        });
    if let Some(frontier) = loaded
        && !schema_only
    {
        let projection = sources::derive_projection(frontier);
        let existing_sources = frontier
            .sources
            .iter()
            .map(|source| source.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let existing_atoms = frontier
            .evidence_atoms
            .iter()
            .map(|atom| atom.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let existing_conditions = frontier
            .condition_records
            .iter()
            .map(|record| record.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for source in projection
            .sources
            .iter()
            .filter(|source| !existing_sources.contains(source.id.as_str()))
        {
            diagnostics.push(json!({
                "severity": "warning",
                "rule_id": "missing_source_record",
                "check": "source_registry",
                "finding_id": source.finding_ids.first(),
                "field_path": "sources",
                "message": format!("Source record {} is derivable but not materialized in frontier state.", source.id),
                "suggestion": "Run `vela frontier materialize` to regenerate derived views before proof export.",
                "fixable": true,
                "normalize_action": "materialize_source_record",
            }));
        }
        for atom in projection
            .evidence_atoms
            .iter()
            .filter(|atom| !existing_atoms.contains(atom.id.as_str()))
        {
            diagnostics.push(json!({
                "severity": "warning",
                "rule_id": "missing_evidence_atom",
                "check": "evidence_atoms",
                "finding_id": atom.finding_id,
                "field_path": "evidence_atoms",
                "message": format!("Evidence atom {} is derivable but not materialized in frontier state.", atom.id),
                "suggestion": "Run `vela frontier materialize .` to rebuild evidence atoms before proof export.",
                "fixable": true,
                "normalize_action": "materialize_evidence_atom",
            }));
        }
        for condition in projection
            .condition_records
            .iter()
            .filter(|condition| !existing_conditions.contains(condition.id.as_str()))
        {
            diagnostics.push(json!({
                "severity": "warning",
                "rule_id": "condition_record_missing",
                "check": "conditions",
                "finding_id": condition.finding_id,
                "field_path": "condition_records",
                "message": format!("Condition record {} is derivable but not materialized in frontier state.", condition.id),
                "suggestion": "Run `vela frontier materialize .` to rebuild condition boundaries before proof export.",
                "fixable": true,
                "normalize_action": "materialize_condition_record",
            }));
        }
        for proposal in frontier.proposals.iter().filter(|proposal| {
            matches!(proposal.status.as_str(), "accepted" | "applied")
                && proposal
                    .reviewed_by
                    .as_deref()
                    .is_none_or(proposals::is_placeholder_reviewer)
        }) {
            diagnostics.push(json!({
                "severity": "error",
                "rule_id": "reviewer_identity_missing",
                "check": "proposals",
                "finding_id": proposal.target.id,
                "field_path": "proposals[].reviewed_by",
                "message": format!("Accepted or applied proposal {} uses a missing or placeholder reviewer identity.", proposal.id),
                "suggestion": "Accept the proposal with a stable named reviewer id before strict proof use.",
                "fixable": false,
                "normalize_action": null,
            }));
        }
    }
    let signal_report = loaded
        .as_ref()
        .map(|frontier| {
            signals::analyze_at(frontier, &diagnostics, Some(frontier_dir_for_source(src)))
        })
        .unwrap_or_else(empty_signal_report);
    let errors = report.errors.len()
        + method_errors
        + graph_errors
        + event_errors
        + state_integrity_errors
        + active_policy_errors.len()
        + policy_readiness_errors.len()
        + activity_leak_errors
        + repository_context_errors;
    let warnings = method_warnings + graph_warnings + signal_report.proof_readiness.warnings;
    let infos = method_infos + graph_infos;
    let strict_blockers = signal_report
        .signals
        .iter()
        .filter(|signal| signal.blocks.iter().any(|block| block == "strict_check"))
        .count();
    let fixable = diagnostics
        .iter()
        .filter(|d| d.get("fixable").and_then(Value::as_bool).unwrap_or(false))
        .count();
    let ok = errors == 0 && (!strict || (warnings == 0 && strict_blockers == 0));

    json!({
        "ok": ok,
        "command": "check",
        "schema_version": project::VELA_SCHEMA_VERSION,
        "source": {
            "path": src.display().to_string(),
            "hash": format!("sha256:{source_hash}"),
        },
        "summary": {
            "status": if ok { "pass" } else { "fail" },
            "checked_findings": report.total_files,
            "valid_findings": report.valid,
            "invalid_findings": report.invalid,
            "errors": errors,
            "warnings": warnings,
            "info": infos,
            "fixable": fixable,
            "strict": strict,
            "schema_only": schema_only,
        },
        "checks": [
            {
                "id": "schema",
                "status": if report.invalid == 0 { "pass" } else { "fail" },
                "checked": report.total_files,
                "failed": report.invalid,
                "errors": report.errors.iter().map(|e| json!({
                    "file": e.file,
                    "message": e.error,
                })).collect::<Vec<_>>(),
            },
            {
                "id": "methodology",
                "status": if method_errors == 0 { "pass" } else { "fail" },
                "checked": method_report.as_ref().map_or(0, |r| r.findings_checked),
                "failed": method_errors,
                "warnings": method_warnings,
                "info": method_infos,
                "skipped": schema_only,
            },
            {
                "id": "frontier_graph",
                "status": if graph_errors == 0 { "pass" } else { "fail" },
                "checked": graph_report.as_ref().map_or(0, |r| r.findings_checked),
                "failed": graph_errors,
                "warnings": graph_warnings,
                "info": graph_infos,
                "skipped": schema_only,
            },
            {
                "id": "signals",
                "status": if strict_blockers == 0 { "pass" } else { "fail" },
                "checked": signal_report.signals.len(),
                "failed": strict_blockers,
                "warnings": signal_report.proof_readiness.warnings,
                "skipped": loaded.is_none(),
                "blockers": signal_report.signals.iter()
                    .filter(|s| s.blocks.iter().any(|b| b == "strict_check"))
                    .map(|s| json!({
                        "id": s.id,
                        "kind": s.kind,
                        "severity": s.severity,
                        "reason": s.reason,
                    }))
                    .collect::<Vec<_>>(),
            },
            {
                "id": "events",
                "status": if replay_report.as_ref().is_none_or(|replay| replay.ok) { "pass" } else { "fail" },
                "checked": replay_report.as_ref().map_or(0, |replay| replay.event_log.count),
                "failed": event_errors,
                "skipped": schema_only || loaded.is_none(),
            },
            {
                "id": "state_integrity",
                "status": if state_integrity_report.as_ref().is_none_or(|report| report.status != "fail") { "pass" } else { "fail" },
                "checked": state_integrity_report.as_ref().map_or(0, |report| report.summary.get("events").copied().unwrap_or_default()),
                "failed": state_integrity_errors,
                "skipped": schema_only || loaded.is_none(),
            },
            {
                "id": "active_policy",
                "status": if active_policy_errors.is_empty() { "pass" } else { "fail" },
                "checked": usize::from(active_policy_result.is_some()),
                "failed": active_policy_errors.len(),
                "errors": active_policy_errors,
                "state": active_policy_assessment.as_ref().map(|assessment| assessment.state().as_str()),
                "permit_readiness": active_policy_assessment.as_ref().map(|assessment| assessment.permit_readiness().as_str()),
                "reason_codes": active_policy_assessment.as_ref().map(|assessment| assessment.reason_codes()).unwrap_or_default(),
                "skipped": active_policy_result.is_none(),
            },
            {
                "id": "policy_readiness",
                "status": if policy_readiness_errors.is_empty() { "pass" } else { "fail" },
                "checked": usize::from(active_policy_assessment.is_some()),
                "failed": policy_readiness_errors.len(),
                "errors": policy_readiness_errors,
                "state": active_policy_assessment.as_ref().map(|assessment| assessment.state().as_str()),
                "permit_readiness": active_policy_assessment.as_ref().map(|assessment| assessment.permit_readiness().as_str()),
                "reason_codes": active_policy_assessment.as_ref().map(|assessment| assessment.reason_codes()).unwrap_or_default(),
                "skipped": active_policy_assessment.as_ref().is_none_or(|assessment| assessment.state()
                    == vela_protocol::proposals::policy_accept::PolicyState::Broken),
            },
            {
                "id": "policy_lane",
                "status": if policy_lane_errors.is_empty() { "pass" } else { "fail" },
                "checked": loaded.map_or(0, |frontier| frontier.events.iter()
                    .filter(|event| event.payload.get(vela_protocol::proposals::policy_accept::POLICY_LANE_PAYLOAD_KEY).is_some())
                    .count()),
                "failed": policy_lane_errors.len(),
                "errors": policy_lane_errors,
                "skipped": !strict || schema_only || loaded.is_none(),
            },
            repository_context.clone(),
        ],
        "event_log": replay_report.as_ref().map(|replay| &replay.event_log),
        "replay": replay_report,
        "state_integrity": state_integrity_report,
        "source_registry": source_registry,
        "evidence_atoms": evidence_atoms,
        "conditions": conditions,
        "proposals": proposal_summary,
        "proof_state": proof_state,
        "repository_context": repository_context,
        "diagnostics": diagnostics,
        "signals": signal_report.signals,
        "review_queue": signal_report.review_queue,
        "proof_readiness": signal_report.proof_readiness,
        "repair_plan": build_repair_plan(&diagnostics),
    })
}

pub(crate) fn hash_path(path: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    if path.is_file() {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("Failed to read {} for hashing: {e}", path.display()))?;
        hasher.update(&bytes);
    } else if path.is_dir() {
        let mut files = Vec::new();
        collect_hash_files(path, path, &mut files)?;
        files.sort();
        for rel in files {
            hasher.update(rel.to_string_lossy().as_bytes());
            let bytes = std::fs::read(path.join(&rel))
                .map_err(|e| format!("Failed to read {} for hashing: {e}", rel.display()))?;
            hasher.update(bytes);
        }
    } else {
        return Err(format!("Cannot hash missing path {}", path.display()));
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn canonical_project_hash(frontier: &project::Project) -> Option<String> {
    let bytes = vela_protocol::canonical::to_canonical_bytes(frontier).ok()?;
    Some(hex::encode(Sha256::digest(bytes)))
}

pub(crate) fn load_frontier_or_fail(path: &Path) -> project::Project {
    repo::load_from_path(path).unwrap_or_else(|e| {
        fail_return(&format!(
            "Failed to load frontier '{}': {e}",
            path.display()
        ))
    })
}

fn collect_hash_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {e}", dir.display()))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_hash_files(root, &path, files)?;
        } else if path.is_file() {
            files.push(
                path.strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn schema_error_suggestion(error: &str) -> &'static str {
    if schema_error_action(error).is_some() {
        "Run `vela frontier materialize .` to rebuild deterministic frontier state."
    } else {
        "Inspect and correct the referenced frontier field."
    }
}

fn schema_error_fix(error: &str) -> bool {
    schema_error_action(error).is_some()
}

fn schema_error_action(error: &str) -> Option<&'static str> {
    if error.contains("stats.findings")
        || error.contains("stats.links")
        || error.contains("Invalid compiler")
        || error.contains("Invalid vela_version")
        || error.contains("Invalid schema")
    {
        Some("normalize_metadata_and_stats")
    } else if error.contains("does not match content-address") {
        Some("rewrite_ids")
    } else {
        None
    }
}

fn build_repair_plan(diagnostics: &[Value]) -> Vec<Value> {
    let mut actions = std::collections::BTreeMap::<String, usize>::new();
    for diagnostic in diagnostics {
        if let Some(action) = diagnostic.get("normalize_action").and_then(Value::as_str) {
            *actions.entry(action.to_string()).or_default() += 1;
        }
    }
    actions
        .into_iter()
        .map(|(action, count)| {
            let command = if action == "rewrite_ids" {
                "vela normalize <frontier> --write --rewrite-ids --id-map id-map.json"
            } else {
                "vela normalize <frontier> --write"
            };
            json!({
                "action": action,
                "count": count,
                "command": command,
            })
        })
        .collect()
}

fn empty_signal_report() -> signals::SignalReport {
    signals::SignalReport {
        schema: "vela.signals.v0".to_string(),
        frontier: "unavailable".to_string(),
        signals: Vec::new(),
        review_queue: Vec::new(),
        proof_readiness: signals::ProofReadiness {
            status: "unavailable".to_string(),
            blockers: 0,
            warnings: 0,
            caveats: vec!["Frontier could not be loaded for signal analysis.".to_string()],
        },
    }
}

#[cfg(test)]
mod repository_context_tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use std::collections::BTreeMap;
    use vela_edge::repository_write::{
        REPOSITORY_TRUST_ANCHOR_SCHEMA_V1, RepositoryTrustAnchorV1,
        install_repository_trust_anchor_from_home,
    };
    use vela_protocol::frontier_profile::FrontierProfileV1;
    use vela_protocol::frontier_repo::{
        FrontierProfileFile, ProfileV1InitOptions, initialize_profile_v1_minimal,
        read_repository_profile,
    };
    use vela_protocol::frontier_repository::{
        FRONTIER_REPOSITORY_BOUNDARY_SCHEMA, FrontierIdentityV1, FrontierRepositoryBoundaryMode,
        FrontierRepositoryBoundaryPayloadV1, FrontierRepositoryTrustMode, GitObjectFormat,
        exact_dependency_root, new_repository_boundary_event,
        repository_boundary_event_content_root, repository_boundary_payload_from_event_shape,
        repository_identity_event_content_root,
    };
    use vela_protocol::sign::{ActorRecord, pubkey_hex, sign_event};

    #[derive(Clone, Copy)]
    enum AnchorCase {
        Valid,
        WrongTree,
        NonAncestor,
    }

    struct BoundRepositoryFixture {
        repo: tempfile::TempDir,
        home: tempfile::TempDir,
        anchor: RepositoryTrustAnchorV1,
    }

    fn git(repo: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
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

    fn profile(repository: &Path) -> FrontierProfileV1 {
        match read_repository_profile(repository).unwrap().unwrap() {
            FrontierProfileFile::V1(profile) => profile,
            FrontierProfileFile::LegacyV0_1(_) => panic!("expected Profile v1"),
        }
    }

    fn bound_repository_with_legacy_parity(
        case: AnchorCase,
        preserve_legacy_parity: bool,
    ) -> BoundRepositoryFixture {
        let repo = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        initialize_profile_v1_minimal(
            repo.path(),
            ProfileV1InitOptions {
                name: "Read context fixture",
                scope: "Does read-side verification fail closed over repository context?",
                initialize_git: false,
            },
        )
        .unwrap();
        let mut project = repo::load_from_path(repo.path()).unwrap();
        let genesis = project.events.first().unwrap().clone();
        let identity = FrontierIdentityV1::from_genesis_event(&genesis).unwrap();
        let key = SigningKey::from_bytes(&[73; 32]);
        let actor = ActorRecord {
            id: "reviewer:repository-administrator".to_string(),
            public_key: pubkey_hex(&key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-23T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        project.actors = vec![actor.clone()];
        if preserve_legacy_parity {
            let mut proposal = vela_protocol::proposals::new_proposal_at(
                "finding.note",
                vela_protocol::events::StateTarget {
                    r#type: "frontier".to_string(),
                    id: project.frontier_id(),
                },
                "agent:legacy",
                "agent",
                "retain exact legacy proposal identity",
                json!({"note": "legacy logical-id preimage"}),
                vec!["src:legacy".to_string()],
                vec!["historical identity is unauthenticated".to_string()],
                "2026-07-22T00:00:30Z",
            );
            proposal.id = "vpr_legacy000000001".to_string();
            project.proposals.push(proposal);
        }
        repo::save_to_path(repo.path(), &project).unwrap();

        git(repo.path(), &["init", "-q", "-b", "main"]);
        git(repo.path(), &["config", "user.name", "Vela Test"]);
        git(
            repo.path(),
            &["config", "user.email", "vela@example.invalid"],
        );
        git(repo.path(), &["add", "."]);
        git(repo.path(), &["commit", "-qm", "repository anchor"]);
        let main_commit = git(repo.path(), &["rev-parse", "HEAD^{commit}"]);
        let anchor_commit = if matches!(case, AnchorCase::NonAncestor) {
            git(repo.path(), &["checkout", "-qb", "fork"]);
            git(
                repo.path(),
                &["commit", "--allow-empty", "-qm", "fork-only anchor"],
            );
            let fork = git(repo.path(), &["rev-parse", "HEAD^{commit}"]);
            git(repo.path(), &["checkout", "-q", "main"]);
            fork
        } else {
            main_commit
        };
        let facts = vela_edge::frontier_repository::derive_repository_anchor_facts(
            repo.path(),
            &anchor_commit,
        )
        .unwrap();
        let mut anchor_tree = facts.git_tree;
        if matches!(case, AnchorCase::WrongTree) {
            anchor_tree = if facts.git_object_format == GitObjectFormat::Sha1 {
                "0".repeat(40)
            } else {
                "0".repeat(64)
            };
        }
        let mut boundary = new_repository_boundary_event(
            FrontierRepositoryBoundaryPayloadV1 {
                schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
                mode: FrontierRepositoryBoundaryMode::UpdateDependencies,
                frontier_id: identity.frontier_id.clone(),
                identity_root: identity.root().unwrap(),
                observed_profile_root: profile(repo.path()).profile_root().unwrap(),
                dependency_root: exact_dependency_root(&[]).unwrap(),
                dependencies: Vec::new(),
                previous_identity_event_root: Some(
                    repository_identity_event_content_root(&genesis).unwrap(),
                ),
                legacy_identity_preimage_root: None,
                administrator_actor_id: actor.id,
                administrator_public_key: actor.public_key,
                administrator_algorithm: actor.algorithm,
                trust_mode: FrontierRepositoryTrustMode::Genesis,
                git_object_format: facts.git_object_format,
                anchor_git_commit: facts.git_commit,
                anchor_git_tree: anchor_tree,
                anchor_event_log_root: facts.event_log_root,
                anchor_event_count: facts.event_count,
                anchor_snapshot_root: facts.snapshot_root,
                anchor_snapshot_schema: facts.snapshot_schema,
                anchor_proposal_root: facts.proposal_root,
                anchor_actor_registry_root: facts.actor_registry_root,
                anchor_artifact_registry_root: facts.artifact_registry_root,
                anchor_canonical_store_root: facts.canonical_store_root,
            },
            "bind exact repository context",
            "2026-07-23T00:01:00Z",
        )
        .unwrap();
        boundary.signature = Some(sign_event(&boundary, &key).unwrap());
        let payload = repository_boundary_payload_from_event_shape(&boundary).unwrap();
        let anchor = RepositoryTrustAnchorV1 {
            schema: REPOSITORY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
            frontier_id: payload.frontier_id,
            identity_root: payload.identity_root,
            boundary_content_root: repository_boundary_event_content_root(&boundary).unwrap(),
            administrator_actor_id: payload.administrator_actor_id,
            administrator_public_key: payload.administrator_public_key,
        };
        project.events.push(boundary);
        repo::save_to_path(repo.path(), &project).unwrap();
        vela_protocol::frontier_repo::materialize(repo.path()).unwrap();
        git(repo.path(), &["add", "."]);
        git(
            repo.path(),
            &[
                "commit",
                "-qm",
                "bind repository and materialize exact views",
            ],
        );

        BoundRepositoryFixture { repo, home, anchor }
    }

    fn bound_repository(case: AnchorCase) -> BoundRepositoryFixture {
        bound_repository_with_legacy_parity(case, false)
    }

    fn snapshot_without_git(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn visit(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in std::fs::read_dir(current).unwrap() {
                let path = entry.unwrap().path();
                if path.file_name().is_some_and(|name| name == ".git") {
                    continue;
                }
                if path.is_dir() {
                    visit(root, &path, files);
                } else if path.is_file() {
                    files.insert(
                        path.strip_prefix(root).unwrap().to_path_buf(),
                        std::fs::read(&path).unwrap(),
                    );
                }
            }
        }
        let mut files = BTreeMap::new();
        visit(root, root, &mut files);
        files
    }

    fn repository_signal(payload: &Value, kind: &str) -> Option<Value> {
        payload
            .get("signals")
            .and_then(Value::as_array)
            .and_then(|signals| {
                signals
                    .iter()
                    .find(|signal| signal.get("kind").and_then(Value::as_str) == Some(kind))
            })
            .cloned()
    }

    #[test]
    fn preloaded_check_preserves_the_complete_strict_payload() {
        let fixture = bound_repository(AnchorCase::Valid);
        install_repository_trust_anchor_from_home(fixture.home.path(), &fixture.anchor).unwrap();
        let project = repo::load_from_path(fixture.repo.path()).unwrap();
        let context = repository_context_assessment_with_project_and_home(
            fixture.repo.path(),
            Some(&project),
            Some(fixture.home.path()),
        );

        let preloaded = check_json_payload_with_preloaded(
            fixture.repo.path(),
            false,
            true,
            Some(&project),
            context.payload,
        );
        let ordinary = check_json_payload_with_home(
            fixture.repo.path(),
            false,
            true,
            Some(fixture.home.path()),
        );

        assert_eq!(preloaded, ordinary);
    }

    #[test]
    fn loaded_source_hash_ignores_noncanonical_checkout_bytes() {
        let fixture = bound_repository(AnchorCase::Valid);
        install_repository_trust_anchor_from_home(fixture.home.path(), &fixture.anchor).unwrap();
        let project = repo::load_from_path(fixture.repo.path()).unwrap();
        let context = repository_context_assessment_with_project_and_home(
            fixture.repo.path(),
            Some(&project),
            Some(fixture.home.path()),
        );
        let before = check_json_payload_with_preloaded(
            fixture.repo.path(),
            false,
            true,
            Some(&project),
            context.payload.clone(),
        );

        std::fs::write(
            fixture.repo.path().join("ignored-check-cache.tmp"),
            b"operational bytes outside the loaded Frontier",
        )
        .unwrap();
        let after = check_json_payload_with_preloaded(
            fixture.repo.path(),
            false,
            true,
            Some(&project),
            context.payload,
        );

        assert_eq!(before["source"]["hash"], after["source"]["hash"]);
        assert_eq!(
            before["source"]["hash"],
            format!(
                "sha256:{}",
                canonical_project_hash(&project).expect("canonical project hash")
            )
        );
    }

    #[test]
    fn strict_nonstrict_invalid_boundary_no_exemption() {
        let fixture = bound_repository(AnchorCase::WrongTree);
        install_repository_trust_anchor_from_home(fixture.home.path(), &fixture.anchor).unwrap();
        let repository_before = snapshot_without_git(fixture.repo.path());
        let trust_before = snapshot_without_git(fixture.home.path());

        let non_strict = check_json_payload_with_home(
            fixture.repo.path(),
            false,
            false,
            Some(fixture.home.path()),
        );
        assert_eq!(
            non_strict
                .pointer("/summary/strict")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            non_strict
                .pointer("/repository_context/status")
                .and_then(Value::as_str),
            Some("fail")
        );
        assert_eq!(
            non_strict
                .pointer("/repository_context/valid")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            non_strict
                .pointer("/repository_context/error")
                .and_then(Value::as_str)
                .is_some_and(|error| error.contains("anchor_git_tree mismatch")),
            "{}",
            non_strict
        );
        let signal = repository_signal(&non_strict, "repository_boundary_invalid").unwrap();
        assert!(
            signal
                .get("blocks")
                .and_then(Value::as_array)
                .is_some_and(|blocks| blocks.iter().any(|block| block == "strict_check"))
        );
        assert!(
            signal
                .get("caveats")
                .and_then(Value::as_array)
                .is_some_and(|caveats| caveats.iter().any(|caveat| caveat
                    .as_str()
                    .is_some_and(|text| text.contains("grants no identity"))))
        );

        let strict = check_json_payload_with_home(
            fixture.repo.path(),
            false,
            true,
            Some(fixture.home.path()),
        );
        assert_eq!(strict.get("ok").and_then(Value::as_bool), Some(false));
        assert_eq!(
            strict
                .pointer("/repository_context/valid")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(repository_signal(&strict, "repository_boundary_invalid").is_some());
        assert_eq!(snapshot_without_git(fixture.repo.path()), repository_before);
        assert_eq!(snapshot_without_git(fixture.home.path()), trust_before);
    }

    #[test]
    fn repository_context_rejects_nonancestor_anchor() {
        let fixture = bound_repository(AnchorCase::NonAncestor);
        install_repository_trust_anchor_from_home(fixture.home.path(), &fixture.anchor).unwrap();
        let repository_before = snapshot_without_git(fixture.repo.path());
        let trust_before = snapshot_without_git(fixture.home.path());
        let checked =
            repository_context_check_with_home(fixture.repo.path(), Some(fixture.home.path()));
        assert_eq!(
            checked.get("code").and_then(Value::as_str),
            Some("repository_boundary_invalid")
        );
        assert!(
            checked
                .get("error")
                .and_then(Value::as_str)
                .is_some_and(|error| error.contains("not an ancestor")),
            "{checked}"
        );
        assert_eq!(snapshot_without_git(fixture.repo.path()), repository_before);
        assert_eq!(snapshot_without_git(fixture.home.path()), trust_before);
    }

    #[test]
    fn repository_context_requires_and_matches_external_pin() {
        let fixture = bound_repository(AnchorCase::Valid);
        let repository_before = snapshot_without_git(fixture.repo.path());
        let empty_home = snapshot_without_git(fixture.home.path());
        let missing =
            repository_context_check_with_home(fixture.repo.path(), Some(fixture.home.path()));
        assert_eq!(
            missing.get("code").and_then(Value::as_str),
            Some("repository_trust_anchor_required")
        );
        let missing_payload = check_json_payload_with_home(
            fixture.repo.path(),
            false,
            false,
            Some(fixture.home.path()),
        );
        assert!(repository_signal(&missing_payload, "repository_trust_anchor_required").is_some());
        assert_eq!(snapshot_without_git(fixture.repo.path()), repository_before);
        assert_eq!(snapshot_without_git(fixture.home.path()), empty_home);

        let mut mismatched = fixture.anchor.clone();
        mismatched.boundary_content_root = format!("sha256:{}", "f".repeat(64));
        install_repository_trust_anchor_from_home(fixture.home.path(), &mismatched).unwrap();
        let mismatched_home = snapshot_without_git(fixture.home.path());
        let mismatch =
            repository_context_check_with_home(fixture.repo.path(), Some(fixture.home.path()));
        assert_eq!(
            mismatch.get("code").and_then(Value::as_str),
            Some("repository_trust_anchor_invalid")
        );
        assert_eq!(mismatch.get("valid").and_then(Value::as_bool), Some(false));
        let mismatch_payload = check_json_payload_with_home(
            fixture.repo.path(),
            false,
            false,
            Some(fixture.home.path()),
        );
        assert!(repository_signal(&mismatch_payload, "repository_trust_anchor_invalid").is_some());
        assert_eq!(snapshot_without_git(fixture.repo.path()), repository_before);
        assert_eq!(snapshot_without_git(fixture.home.path()), mismatched_home);
    }

    #[test]
    fn repository_context_accepts_the_exact_external_pin_without_writes() {
        let fixture = bound_repository(AnchorCase::Valid);
        let installed =
            install_repository_trust_anchor_from_home(fixture.home.path(), &fixture.anchor)
                .unwrap();
        let repository_before = snapshot_without_git(fixture.repo.path());
        let trust_before = snapshot_without_git(fixture.home.path());

        let checked =
            repository_context_check_with_home(fixture.repo.path(), Some(fixture.home.path()));
        assert_eq!(checked.get("status").and_then(Value::as_str), Some("pass"));
        assert_eq!(checked.get("valid").and_then(Value::as_bool), Some(true));
        assert_eq!(
            checked.get("identity_mode").and_then(Value::as_str),
            Some("pinned_boundary")
        );
        assert_eq!(
            checked.get("trust_anchor_root").and_then(Value::as_str),
            Some(installed.root.as_str())
        );
        assert_eq!(snapshot_without_git(fixture.repo.path()), repository_before);
        assert_eq!(snapshot_without_git(fixture.home.path()), trust_before);
    }

    #[test]
    fn github_action_profile_v1_strict_check_uses_exact_pin_without_frontier_writes() {
        let fixture = bound_repository(AnchorCase::Valid);
        install_repository_trust_anchor_from_home(fixture.home.path(), &fixture.anchor).unwrap();
        let repository_before = snapshot_without_git(fixture.repo.path());
        let trust_before = snapshot_without_git(fixture.home.path());

        let checked = check_json_payload_with_home(
            fixture.repo.path(),
            false,
            true,
            Some(fixture.home.path()),
        );
        assert_eq!(
            checked.get("ok").and_then(Value::as_bool),
            Some(true),
            "{checked}"
        );
        assert_eq!(
            checked
                .pointer("/repository_context/identity_mode")
                .and_then(Value::as_str),
            Some("pinned_boundary")
        );
        assert_eq!(
            checked
                .pointer("/repository_context/valid")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(snapshot_without_git(fixture.repo.path()), repository_before);
        assert_eq!(snapshot_without_git(fixture.home.path()), trust_before);
    }

    #[test]
    fn strict_check_reports_exact_anchored_legacy_proposal_identity_without_blocking() {
        let fixture = bound_repository_with_legacy_parity(AnchorCase::Valid, true);
        install_repository_trust_anchor_from_home(fixture.home.path(), &fixture.anchor).unwrap();
        let repository_before = snapshot_without_git(fixture.repo.path());
        let trust_before = snapshot_without_git(fixture.home.path());

        let checked = check_json_payload_with_home(
            fixture.repo.path(),
            false,
            true,
            Some(fixture.home.path()),
        );
        assert_eq!(
            checked.get("ok").and_then(Value::as_bool),
            Some(true),
            "{checked}"
        );
        assert_eq!(
            checked
                .pointer("/repository_context/legacy_proposal_identity_debt/classification")
                .and_then(Value::as_str),
            Some("anchored_immutable_unauthenticated")
        );
        assert_eq!(
            checked
                .pointer("/repository_context/legacy_proposal_identity_debt/count")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert!(preserves_anchored_legacy_proposal_parity(
            &checked["repository_context"],
            1,
        ));
        assert!(!proposal_parity_blocks(&checked["repository_context"], 1,));
        assert_eq!(
            proposal_parity_summary(&checked["repository_context"], 1),
            "1 anchored immutable legacy identity record(s) (unauthenticated)"
        );
        assert!(
            checked
                .get("diagnostics")
                .and_then(Value::as_array)
                .is_some_and(|diagnostics| diagnostics.iter().any(|diagnostic| {
                    diagnostic.get("rule_id").and_then(Value::as_str)
                        == Some("anchored_legacy_proposal_identity")
                        && diagnostic.get("severity").and_then(Value::as_str) == Some("info")
                })),
            "{checked}"
        );
        assert!(
            checked
                .get("signals")
                .and_then(Value::as_array)
                .is_some_and(|signals| signals.iter().all(|signal| {
                    signal
                        .get("kind")
                        .and_then(Value::as_str)
                        .is_none_or(|kind| kind != "check_error")
                })),
            "{checked}"
        );
        assert_eq!(snapshot_without_git(fixture.repo.path()), repository_before);
        assert_eq!(snapshot_without_git(fixture.home.path()), trust_before);
    }

    #[test]
    fn predecessor_logical_id_conflict_points_to_historical_inspection_not_reissue() {
        let legacy_context = json!({"generation": "legacy_v0_1"});
        let suggestion = proposal_parity_suggestion(
            &legacy_context,
            "proposal vpr_legacy logical content derives id vpr_current",
        );
        assert!(suggestion.contains("Do not rewrite or re-issue"));
        assert!(suggestion.contains("historical Vela release"));
        assert!(!suggestion.contains("vela sign"));

        let native_context = json!({"generation": "profile_v1"});
        let native_suggestion = proposal_parity_suggestion(
            &native_context,
            "proposal vpr_native logical content derives id vpr_other",
        );
        assert!(native_suggestion.contains("vela review accept"));
    }

    #[test]
    fn strict_check_grants_no_legacy_proposal_exemption_after_tampering() {
        let fixture = bound_repository_with_legacy_parity(AnchorCase::Valid, true);
        install_repository_trust_anchor_from_home(fixture.home.path(), &fixture.anchor).unwrap();
        let mut project = repo::load_from_path(fixture.repo.path()).unwrap();
        project.proposals[0].reason = "tampered after the signed anchor".to_string();
        repo::save_to_path(fixture.repo.path(), &project).unwrap();

        let checked = check_json_payload_with_home(
            fixture.repo.path(),
            false,
            true,
            Some(fixture.home.path()),
        );
        assert_eq!(checked.get("ok").and_then(Value::as_bool), Some(false));
        assert_eq!(
            checked
                .pointer("/repository_context/valid")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            checked
                .pointer("/repository_context/error")
                .and_then(Value::as_str)
                .is_some_and(|error| {
                    error.contains(
                        "introduced parity failures absent from the exact repository anchor",
                    )
                }),
            "{checked}"
        );
        assert!(
            checked
                .get("diagnostics")
                .and_then(Value::as_array)
                .is_some_and(|diagnostics| diagnostics.iter().any(|diagnostic| {
                    diagnostic.get("rule_id").and_then(Value::as_str)
                        == Some("review_decision_parity")
                        && diagnostic.get("severity").and_then(Value::as_str) == Some("error")
                })),
            "{checked}"
        );
        assert!(
            checked
                .pointer("/repository_context/legacy_proposal_identity_debt")
                .is_none()
        );
    }

    #[test]
    fn github_action_profile_v1_genesis_strict_check_needs_no_pin() {
        let repository = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        initialize_profile_v1_minimal(
            repository.path(),
            ProfileV1InitOptions {
                name: "Action genesis fixture",
                scope: "Can strict CI verify structural genesis without inventing an administrator?",
                initialize_git: true,
            },
        )
        .unwrap();
        vela_protocol::frontier_repo::materialize(repository.path()).unwrap();
        let repository_before = snapshot_without_git(repository.path());
        let home_before = snapshot_without_git(home.path());

        let checked =
            check_json_payload_with_home(repository.path(), false, true, Some(home.path()));
        assert_eq!(
            checked.get("ok").and_then(Value::as_bool),
            Some(true),
            "{checked}"
        );
        assert_eq!(
            checked
                .pointer("/repository_context/identity_mode")
                .and_then(Value::as_str),
            Some("genesis")
        );
        assert_eq!(snapshot_without_git(repository.path()), repository_before);
        assert_eq!(snapshot_without_git(home.path()), home_before);
    }
}
