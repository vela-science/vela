//! The setup/ceremony lane of `vela doctor`.
//!
//! The dev-oriented checks live in vela-edge (`doctor::run`); these are
//! the operator-machine checks — identity, key custody, binary pin, hub
//! policy freshness, adapter sync, registry health. Every one of
//! them encodes an incident that actually happened; each carries the ONE
//! command that fixes it. Merged into the doctor report at the cmd layer
//! (vela-edge cannot see these crate-local stores).

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
    out.push(pin_check());
    if let Some(dir) = frontier {
        out.push(policy_check(dir));
        out.push(adapters_check(dir));
    }
    out.push(registry_check());
    out
}

/// Identity + key custody: the file exists and is not world-readable.
fn identity_check() -> SetupCheck {
    let Some(id) = super::cli_identity::load_identity() else {
        return warn(
            "identity",
            "no identity on this machine",
            "vela id create --handle <you>",
        );
    };
    let key = Path::new(&id.key_path);
    if !key.exists() {
        return fail(
            "identity",
            format!(
                "{} names a key that does not exist: {}",
                id.actor_id, id.key_path
            ),
            "vela id create --handle <you>  (or restore the key file)",
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(key)
            && meta.permissions().mode() & 0o077 != 0
        {
            return warn(
                "identity",
                format!("{} key is readable beyond your user", id.actor_id),
                format!("chmod 600 {}", id.key_path),
            );
        }
    }
    ok("identity", id.actor_id)
}

/// Binary pin: recorded, matching, and not a workshop build.
///
/// The workshop heuristic encodes a real incident: cargo hardlink-swaps
/// `target/release` binaries between build environments, so a pin on a
/// dev-tree binary churns on every alternation. The pen should be an
/// installed release.
fn pin_check() -> SetupCheck {
    let exe_in_target = std::env::current_exe()
        .ok()
        .map(|p| p.components().any(|c| c.as_os_str() == "target"))
        .unwrap_or(false);
    match super::binary_pin::verify_for_ceremony() {
        Err(e) => fail(
            "binary pin",
            e,
            "vela id pin-binary  (after inspecting the binary)",
        ),
        Ok(None) => warn(
            "binary pin",
            "unpinned — ceremonies run without a binary anchor",
            "vela id pin-binary",
        ),
        Ok(Some(pin)) if exe_in_target => warn(
            "binary pin",
            format!(
                "pinned {} but this is a workshop build (cargo target dir) — the pin will churn",
                &pin.sha256[..12]
            ),
            "cargo install --path crates/vela-cli --locked --force && vela id pin-binary",
        ),
        Ok(Some(pin)) => ok(
            "binary pin",
            format!("pinned {} ({})", &pin.sha256[..12], pin.version),
        ),
    }
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
            let repair = if assessment.reason_codes().iter().any(|code| {
                matches!(
                    code.as_str(),
                    "policy_wall_clock_expiry_unanchored" | "policy_expired"
                )
            }) {
                "draft and sign a replacement policy with causal Permit authority; re-signing unchanged bytes does not renew them"
            } else if assessment.reason_codes().iter().any(|code| {
                matches!(
                    code.as_str(),
                    "policy_head_missing"
                        | "policy_head_mismatch"
                        | "policy_head_revoked"
                        | "policy_revoked"
                        | "policy_authority_invalid"
                )
            }) {
                "vela policy draft <template> --replace  (then one human `vela policy sign`)"
            } else if assessment
                .reason_codes()
                .iter()
                .any(|code| code == "policy_unsigned")
            {
                "vela policy sign ."
            } else if assessment
                .reason_codes()
                .iter()
                .any(|code| code == "policy_absent")
            {
                "vela policy suggest ."
            } else {
                "inspect `vela policy show --json` and complete the required human policy-head or authority ceremony"
            };
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

/// Registry health: how many registered frontiers still exist on disk.
fn registry_check() -> SetupCheck {
    let all = super::workspace_registry::load().frontiers;
    let gone: Vec<&str> = all
        .iter()
        .filter(|f| !Path::new(&f.path).join(".vela").is_dir())
        .map(|f| f.path.as_str())
        .collect();
    if gone.is_empty() {
        ok("registry", format!("{} live frontier(s)", all.len()))
    } else {
        // Show enough to recognize the debris, never a wall of temp paths.
        let shown = gone
            .iter()
            .take(3)
            .map(|p| p.rsplit('/').next().unwrap_or(p))
            .collect::<Vec<_>>()
            .join(", ");
        let more = gone.len().saturating_sub(3);
        let tail = if more > 0 {
            format!(" (+{more} more)")
        } else {
            String::new()
        };
        warn(
            "registry",
            format!(
                "{} of {} registered frontier(s) no longer exist: {shown}{tail} — compacts on the next registration",
                gone.len(),
                all.len(),
            ),
            String::new(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The workshop heuristic must fire for the test binary itself —
    /// cargo test runs from target/, which is exactly the shape the
    /// check warns about (given a valid pin; without one it's the
    /// unpinned warn). Either way pin_check never says Ok here.
    #[test]
    fn test_binary_is_never_an_ok_ceremony_anchor() {
        let c = pin_check();
        assert_ne!(
            c.status,
            SetupStatus::Ok,
            "a target-dir binary must not be a clean pin: {}",
            c.detail
        );
    }

    #[test]
    fn registry_and_identity_checks_never_panic() {
        let _ = registry_check();
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
