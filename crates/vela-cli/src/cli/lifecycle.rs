//! Frontier lifecycle: `vela init` (with versioned git hooks) and the
//! `vela serve --setup` MCP scaffold. Moved verbatim from `cli/mod.rs`.

use super::*;

pub(crate) fn cmd_init(path: &Path, name: Option<&str>, scope: Option<&str>, json_output: bool) {
    if path.join(".vela").exists() {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Exists,
            &format!(
                "already initialized: {} exists",
                path.join(".vela").display()
            ),
            Some("run `vela status` to see the frontier that already lives here"),
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
                Some("provide an explicit bounded frontier name and scope"),
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
    let name = resolve("name", name);
    let scope = resolve("scope", scope);
    let mut payload = frontier_repo::initialize_minimal(
        path,
        frontier_repo::InitOptions {
            name: &name,
            initialize_git: true,
        },
    )
    .unwrap_or_else(|e| fail_return(&e));
    std::fs::write(
        path.join("SCOPE.md"),
        format!("# Scope\n\n## Question\n\n{scope}\n\n## Excludes\n\nAnything not required to answer the bounded question above.\n"),
    )
    .unwrap_or_else(|error| fail_return(&format!("write SCOPE.md: {error}")));
    payload["scope"] = serde_json::json!(scope);
    payload["wrote"] = serde_json::json!([
        "README.md",
        "SCOPE.md",
        "frontier.yaml",
        "frontier.json",
        "vela.lock",
        ".gitignore",
        ".gitattributes",
        "VELA.md"
    ]);
    payload["next_commands"] = serde_json::json!([
        format!("vela doctor '{}' --json", path.display()),
        format!("vela status '{}' --json", path.display()),
        format!("vela next '{}' --json", path.display())
    ]);
    crate::config::workspace_registry::register(path, Some(&name));
    let hooks = scaffold_git_hooks(path);
    if json_output {
        print_json(&payload);
    } else {
        println!(
            "{} initialized frontier repository in {}",
            style::ok("ok"),
            path.display()
        );
        if hooks {
            println!("  git hooks installed (.vela/hooks): pre-push runs the strict check");
        }
    }
}

/// Versioned git hooks: local CI before the Action sees the push, and
/// derived views that can never lag the committed store. Written under
/// `.vela/hooks` (committed with the repo) and activated via
/// `core.hooksPath`; a clone re-activates with one config line, which
/// `vela doctor` suggests.
fn scaffold_git_hooks(path: &Path) -> bool {
    if !path.join(".git").exists() {
        return false;
    }
    let hooks_dir = path.join(".vela/hooks");
    if std::fs::create_dir_all(&hooks_dir).is_err() {
        return false;
    }
    let pre_commit = r#"#!/bin/sh
# vela pre-commit: the committed store must never lead its derived views
# (CI holds them to hash parity). If events are staged, re-materialize
# and stage the views alongside them.
if git diff --cached --name-only | grep -q "\.vela/events/"; then
  if command -v vela >/dev/null 2>&1; then
    root="$(git rev-parse --show-toplevel)"
    vela frontier materialize "$root" >/dev/null 2>&1 &&       git add "$root/frontier.json" "$root/vela.lock" "$root/proof" 2>/dev/null
  fi
fi
exit 0
"#;
    let pre_push = r#"#!/bin/sh
# vela pre-push: hold the push to the same strict bar CI will.
command -v vela >/dev/null 2>&1 || exit 0
root="$(git rev-parse --show-toplevel)"
if ! vela check "$root" --strict >/dev/null 2>&1; then
  echo "vela pre-push: strict check failed — push aborted."
  echo "  inspect: vela check $root --strict"
  echo "  bypass (CI will still refuse): git push --no-verify"
  exit 1
fi
exit 0
"#;
    let ok = std::fs::write(hooks_dir.join("pre-commit"), pre_commit).is_ok()
        && std::fs::write(hooks_dir.join("pre-push"), pre_push).is_ok();
    if !ok {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for name in ["pre-commit", "pre-push"] {
            let _ = std::fs::set_permissions(
                hooks_dir.join(name),
                std::fs::Permissions::from_mode(0o755),
            );
        }
    }
    std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["config", "core.hooksPath", ".vela/hooks"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub(crate) fn cmd_mcp_setup(source: Option<&Path>, frontiers: Option<&Path>) {
    let source_desc = source
        .map(|p| p.display().to_string())
        .or_else(|| frontiers.map(|p| p.display().to_string()))
        .unwrap_or_else(|| "frontier.json".to_string());
    // Emit the read-only profile by default (memo §9.1): the safe generic MCP
    // snippet. A frontier's generated `.mcp.json` opts into the nonfinalizing
    // draft profile explicitly so `next -> work -> land` remains available.
    let args = if let Some(path) = source {
        format!(r#""serve", "{}", "--profile", "read-only""#, path.display())
    } else if let Some(path) = frontiers {
        format!(
            r#""serve", "--frontiers", "{}", "--profile", "read-only""#,
            path.display()
        )
    } else {
        r#""serve", "frontier.json", "--profile", "read-only""#.to_string()
    };
    println!(
        r#"Add this MCP server configuration to your client:

{{
  "mcpServers": {{
    "vela": {{
      "command": "vela",
      "args": [{args}]
    }}
  }}
}}

Source: {source_desc}"#
    );
}
