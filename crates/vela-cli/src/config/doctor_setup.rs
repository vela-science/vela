//! The local setup lane of `vela doctor`.
//!
//! The dev-oriented checks live in vela-edge (`doctor::run`); these are
//! the operator-machine checks for an optional producer identity, policy
//! readiness, and adapter sync. Human decisions use the local OS principal and
//! repository authority; there is no helper, personal signing identity, or
//! host-global binary-pin setup.

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SetupStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SetupCheck {
    pub name: &'static str,
    pub status: SetupStatus,
    pub detail: String,
    /// The one fixing command; empty when status is Ok.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub next: String,
}

fn ok(name: &'static str, detail: impl Into<String>) -> SetupCheck {
    SetupCheck {
        name,
        status: SetupStatus::Ok,
        detail: detail.into(),
        next: String::new(),
    }
}

fn warn(name: &'static str, detail: impl Into<String>, next: impl Into<String>) -> SetupCheck {
    SetupCheck {
        name,
        status: SetupStatus::Warn,
        detail: detail.into(),
        next: next.into(),
    }
}

fn fail(name: &'static str, detail: impl Into<String>, next: impl Into<String>) -> SetupCheck {
    SetupCheck {
        name,
        status: SetupStatus::Fail,
        detail: detail.into(),
        next: next.into(),
    }
}

/// Run every setup check using local state only. Network reachability belongs
/// to explicit hub commands, not the first-run frontier diagnostic.
pub(crate) fn run(frontier: Option<&Path>) -> Vec<SetupCheck> {
    let mut out = Vec::new();
    out.push(identity_check());
    if let Some(dir) = frontier {
        out.push(policy_check(dir));
        out.push(adapters_check(dir));
    }
    out
}

/// Producer identity and key custody. Human authority needs no local identity.
fn identity_check() -> SetupCheck {
    let Some(id) = super::cli_identity::load_identity() else {
        return warn(
            "producer identity",
            "no agent identity configured; read, reproduce, and human review remain available",
            "vela id create --agent --handle <name>",
        );
    };
    identity_check_loaded(&id)
}

fn identity_check_loaded(id: &super::cli_identity::Identity) -> SetupCheck {
    if id.actor_type == "human" || id.actor_id.starts_with("reviewer:") {
        return warn(
            "producer identity",
            format!(
                "{} is a retired local human-signing profile and is ignored by repository authority",
                id.actor_id
            ),
            format!("rm {}", super::cli_identity::identity_path().display()),
        );
    }

    let key_path = &id.key_path;
    let key = Path::new(key_path);
    if !key.exists() {
        return fail(
            "producer identity",
            format!(
                "{} names a key that does not exist: {}",
                id.actor_id, key_path
            ),
            "vela id create --agent --handle <name> --force  (or restore the agent key file)",
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(key)
            && meta.permissions().mode() & 0o077 != 0
        {
            return warn(
                "producer identity",
                format!("{} key is readable beyond your user", id.actor_id),
                format!("chmod 600 {key_path}"),
            );
        }
    }
    ok("producer identity", id.actor_id.clone())
}

/// Active-policy bytes and standing Permit authority for the cwd frontier.
fn policy_check(dir: &Path) -> SetupCheck {
    let snapshot = vela_protocol::acceptance_policy::load_active_policy_snapshot(dir);
    let project = match vela_protocol::repo::load_from_path(dir) {
        Ok(project) => project,
        Err(error) => return fail("policy", error, "vela check ."),
    };
    let assessment = vela_protocol::proposals::policy_accept::assess_policy_readiness(
        &project,
        snapshot.as_ref().map_err(String::as_str),
        &chrono::Utc::now().to_rfc3339(),
    );
    let summary = format!(
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
        vela_protocol::proposals::policy_accept::PermitReadiness::Ready => ok("policy", summary),
        vela_protocol::proposals::policy_accept::PermitReadiness::Blocked => fail(
            "policy",
            assessment.detail().unwrap_or(&summary),
            "inspect the active policy pair and policy-head chain; invalid governance never fails open",
        ),
        vela_protocol::proposals::policy_accept::PermitReadiness::HumanOnly => {
            let repair = "inspect `vela policy show --json`; frozen Era-0 policy bytes remain replayable, while new authority requires the repository-authority migration";
            warn("policy", summary, repair)
        }
    }
}

/// Adapter sync for the cwd frontier (`vela agents doctor` as one row).
fn adapters_check(dir: &Path) -> SetupCheck {
    let (in_sync, drifted, missing) = match super::cli_agents::try_compare(dir) {
        Ok(comparison) => comparison,
        Err(error) => {
            return warn(
                "adapters",
                error,
                format!("vela agents sync {}", dir.display()),
            );
        }
    };
    if drifted.is_empty() && missing.is_empty() {
        ok("adapters", format!("{} adapter(s) in sync", in_sync.len()))
    } else {
        warn(
            "adapters",
            format!("{} drifted, {} missing", drifted.len(), missing.len()),
            "vela agents sync",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retired_human_identity() -> super::super::cli_identity::Identity {
        super::super::cli_identity::Identity {
            version: "2.0".to_string(),
            actor_id: "reviewer:test".to_string(),
            actor_type: "human".to_string(),
            key_path: "/removed/private.key".to_string(),
            pubkey: "11".repeat(32),
        }
    }

    #[test]
    fn retired_human_identity_is_ignored_and_names_cleanup() {
        let check = identity_check_loaded(&retired_human_identity());
        assert_eq!(check.status, SetupStatus::Warn, "{}", check.detail);
        assert!(check.detail.contains("retired"));
        assert!(check.next.starts_with("rm "));
    }

    #[test]
    fn identity_check_never_panics() {
        let _ = identity_check();
    }

    #[test]
    fn malformed_unsigned_policy_is_reported_unreadable_not_staged() {
        let temp = tempfile::tempdir().unwrap();
        let policies = temp.path().join(".vela/policies");
        std::fs::create_dir_all(&policies).unwrap();
        std::fs::write(policies.join("active.json"), b"{not-json\n").unwrap();

        let check = policy_check(temp.path());
        assert_eq!(check.status, SetupStatus::Fail);
        assert!(check.detail.contains("active policy parse"));
        assert!(!check.detail.contains("unsigned policy draft"));
    }

    #[test]
    fn missing_agent_charter_is_a_doctor_warning_not_a_process_exit() {
        let temp = tempfile::tempdir().unwrap();
        let check = adapters_check(temp.path());
        assert_eq!(check.status, SetupStatus::Warn);
        assert!(check.detail.contains("no VELA.md"), "{}", check.detail);
        assert!(check.next.contains("vela agents sync"));
    }
}
