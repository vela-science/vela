//! Frontier lifecycle initialization.

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
    let payload = frontier_repo::initialize_profile_v1_minimal(
        path,
        frontier_repo::ProfileV1InitOptions {
            name: &name,
            scope: &scope,
            initialize_git: true,
        },
    )
    .unwrap_or_else(|e| fail_return(&e));
    if json_output {
        print_json(&payload);
    } else {
        println!(
            "{} initialized frontier repository in {}",
            style::ok("ok"),
            path.display()
        );
    }
}
