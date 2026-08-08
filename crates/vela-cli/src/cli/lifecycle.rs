//! Repository lifecycle initialization.

use super::*;
use serde_json::json;

pub(crate) fn cmd_init(
    path: &Path,
    name: Option<&str>,
    scope: Option<&str>,
    key_selector: Option<&str>,
    reason: &str,
    json_output: bool,
) {
    crate::ui::set_mode("init", json_output);
    let store = path.join(".vela");
    let initialized =
        store.join("origin.json").is_file() || store.join("repository.json").is_file();
    if initialized {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Exists,
            &format!("repository is already initialized at {}", path.display()),
            Some("run `vela status` to see the repository that already lives here"),
        );
    }
    let resolve = |label: &str, supplied: Option<&str>| -> String {
        if let Some(value) = supplied.map(str::trim).filter(|value| !value.is_empty()) {
            return value.to_string();
        }
        if json_output {
            crate::ui::fail_with(
                crate::ui::ErrorKind::Usage,
                &format!("init --json requires --{label}"),
                Some("provide an explicit bounded repository name and scope"),
            );
        }
        print!("{label}: ");
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let mut value = String::new();
        std::io::stdin()
            .read_line(&mut value)
            .unwrap_or_else(|error| fail_return(&format!("read {label}: {error}")));
        let value = value.trim();
        if value.is_empty() {
            crate::ui::fail_with(
                crate::ui::ErrorKind::Usage,
                &format!("init requires a non-empty {label}"),
                Some("run again and provide a bounded value"),
            );
        }
        value.to_string()
    };
    let mut payload = if store.exists() {
        let profile = crate::repository::verify_bootstrap_at(path)
            .unwrap_or_else(|error| fail_return(&error));
        if let Some(supplied) = name.map(str::trim).filter(|value| !value.is_empty())
            && supplied != profile.name
        {
            crate::ui::fail_with(
                crate::ui::ErrorKind::Usage,
                "--name does not match the retained repository profile",
                Some("omit --name when resuming `vela init`, or pass the exact retained value"),
            );
        }
        if let Some(supplied) = scope.map(str::trim).filter(|value| !value.is_empty())
            && supplied != profile.scope.question
        {
            crate::ui::fail_with(
                crate::ui::ErrorKind::Usage,
                "--scope does not match the retained repository profile",
                Some("omit --scope when resuming `vela init`, or pass the exact retained value"),
            );
        }
        json!({
            "schema": "vela.repository-init-draft.v1",
            "ok": true,
            "layout": "vela.repository-bootstrap.v1",
            "path": path.display().to_string(),
            "name": profile.name,
            "scope": profile.scope.question,
            "repository_id": profile.repository_id,
            "profile_root": profile.profile_root()
                .unwrap_or_else(|error| fail_return(&error)),
            "authority": "uninitialized",
            "scientific_object_count": 0,
            "wrote": [],
            "resumed": true,
        })
    } else {
        let name = resolve("name", name);
        let scope = resolve("scope", scope);
        let mut initialized = crate::init::initialize_minimal(
            path,
            crate::init::InitOptions {
                name: &name,
                scope: &scope,
                initialize_git: true,
            },
        )
        .unwrap_or_else(|error| fail_return(&error));
        initialized["resumed"] = json!(false);
        initialized
    };
    let authority = initialize_repository_authority(path, key_selector, reason)
        .unwrap_or_else(|error| match error {
            RepositoryAuthorityInitError::Signing(error) => {
                let recovery =
                    authority_recovery_hint(path, key_selector, reason, json_output, &error);
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Domain,
                    &format!(
                        "repository profile retained at {}, but signing could not complete: {error}",
                        path.display()
                    ),
                    Some(&recovery),
                )
            }
            RepositoryAuthorityInitError::TrustPinCollision {
                repository_id,
                record_root,
                pin_path,
                pinned_root,
            } => crate::ui::fail_with(
                crate::ui::ErrorKind::Domain,
                &format!(
                    "repository authority initialized at {record_root}, but the local trust pin for {repository_id} at {pin_path} already selects {pinned_root}"
                ),
                Some(&trust_pin_collision_hint(
                    path,
                    &repository_id,
                    &record_root,
                    &pin_path,
                    &pinned_root,
                    json_output,
                )),
            ),
        });
    payload["schema"] = json!("vela.repository-init.v1");
    payload["authority"] = json!({
        "state": "initialized",
        "principal_id": authority["principal_id"],
        "key_id": authority["repository_key_id"],
        "key_fingerprint": authority["repository_key_fingerprint"],
        "record_id": authority["authority_record_id"],
        "record_root": authority["authority_record_root"],
        "keyset_root": authority["authority_keyset_root"],
        "policy_root": authority["policy_bundle_root"],
        "local_trust": authority["local_trust"],
    });
    payload["repository"] = json!({
        "origin_id": authority["origin_id"],
        "origin_root": authority["origin_root"],
        "repository_root": authority["repository_root"],
        "git_commit": authority["git_commit"],
        "git_tree": authority["git_tree"],
    });
    payload["next_action"] = json!(format!(
        "vela submit --repo {} --help",
        shell_arg(&path.display().to_string())
    ));
    if json_output {
        print_json(&payload);
    } else {
        println!(
            "{} initialized signed repository in {}",
            style::ok("ok"),
            path.display()
        );
        println!(
            "  authority {}",
            payload["authority"]["key_fingerprint"]
                .as_str()
                .unwrap_or("unavailable")
        );
        println!(
            "  root      {}",
            payload["repository"]["repository_root"]
                .as_str()
                .unwrap_or("unavailable")
        );
        println!(
            "  commit    {}",
            payload["repository"]["git_commit"]
                .as_str()
                .unwrap_or("unavailable")
        );
        println!(
            "  next      {}",
            payload["next_action"].as_str().unwrap_or("vela status")
        );
    }
}

fn shell_arg(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn resume_command(
    path: &Path,
    key_selector: Option<&str>,
    reason: &str,
    json_output: bool,
) -> String {
    let mut command = format!("vela init {}", shell_arg(&path.display().to_string()));
    if let Some(key) = key_selector {
        command.push_str(&format!(" --key {}", shell_arg(key)));
    }
    command.push_str(&format!(" --reason {}", shell_arg(reason)));
    if json_output {
        command.push_str(" --json");
    }
    command
}

/// The remedy for a pin collision is never a key operation: a new key produces
/// a new record root, hence the same collision. It is also not automatically a
/// rebind — repository_id is derived from name and scope, so the installed pin
/// may belong to a different repository that chose the same two.
fn trust_pin_collision_hint(
    path: &Path,
    repository_id: &str,
    record_root: &str,
    pin_path: &str,
    pinned_root: &str,
    json_output: bool,
) -> String {
    let mut rebind = format!(
        "vela authority trust pin {} --record-root {record_root} --previous-record-root {pinned_root}",
        shell_arg(&path.display().to_string())
    );
    if json_output {
        rebind.push_str(" --json");
    }
    format!(
        "the pin is a write gate, so this repository cannot take an authority write until it is reconciled; read {pin_path} first. If it pins a different repository, rerun `vela init` with a --name or --scope that does not derive {repository_id}. If it is a stale pin for this repository and you have independently verified the new root, advance it with: {rebind}"
    )
}

fn authority_recovery_hint(
    path: &Path,
    key_selector: Option<&str>,
    reason: &str,
    json_output: bool,
    error: &str,
) -> String {
    const SETUP: &str = "https://github.com/vela-science/vela/blob/main/docs/QUICKSTART.md#first-time-authority-key-setup";
    let resume = resume_command(path, key_selector, reason, json_output);
    if error.contains("multiple Ed25519 identities") {
        return format!(
            "choose one listed full fingerprint with --key, then rerun: {resume}; key setup: {SETUP}"
        );
    }
    format!(
        "load one dedicated Ed25519 key with ssh-add /path/to/private-key (start ssh-agent first on Linux), then rerun: {resume}; key setup: {SETUP}"
    )
}
