//! Typed finding-link editing (`vela finding link`). Moved verbatim from
//! `cli/mod.rs`.

use super::*;

async fn cross_frontier_target_warning(
    locator: &str,
    target_vfr: &str,
    target_vf: &str,
) -> Option<String> {
    // This helper runs inside the CLI's Tokio runtime. Use reqwest's native
    // async client so its runtime lifecycle remains owned by the CLI.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;
    let resp = client.get(locator).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let dep_project = resp.json::<vela_protocol::project::Project>().await.ok()?;
    if let Some(target_finding) = dep_project.findings.iter().find(|f| f.id == target_vf) {
        if target_finding.flags.superseded {
            return Some(format!(
                "warn · cross-frontier target '{target_vf}' in '{target_vfr}' has flags.superseded = true. \
You may be linking to outdated wording. Inspect the supersedes chain to find the current finding. \
Use --no-check-target to skip this check."
            ));
        }
        None
    } else {
        Some(format!(
            "warn · cross-frontier target '{target_vf}' not found in dep '{target_vfr}' (fetched from {locator}). \
The target may have been removed or never existed in the pinned snapshot."
        ))
    }
}

/// v0.8: frontier-level metadata commands. Manages cross-frontier
/// dependency declarations on a frontier file. The substrate enforces
/// that any link target of the form `vf_…@vfr_…` references a declared
/// dependency; these commands edit the declaration list.
/// v0.9: typed link addition. Until v0.9 the only way to add a link
/// was to hand-edit JSON; this command is the CLI on-ramp. Links go
/// directly onto `findings[i].links` (links are not a state-changing
/// proposal kind in v0).
pub(crate) async fn cmd_link(action: LinkAction) {
    use vela_protocol::bundle::{Link, LinkRef};
    match action {
        LinkAction::Add {
            frontier,
            from,
            to,
            r#type,
            note,
            inferred_by,
            no_check_target,
            json,
        } => {
            validate_enum_arg("--type", &r#type, bundle::VALID_LINK_TYPES);
            if !["compiler", "reviewer", "author"].contains(&inferred_by.as_str()) {
                fail(&format!(
                    "invalid --inferred-by '{inferred_by}'. Valid: compiler, reviewer, author"
                ));
            }
            let parsed = LinkRef::parse(&to).unwrap_or_else(|e| {
                fail(&format!(
                    "invalid --to '{to}': {e}. Expected vf_<hex> or vf_<hex>@vfr_<hex>"
                ))
            });
            let mut p = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let source_idx = p
                .findings
                .iter()
                .position(|f| f.id == from)
                .unwrap_or_else(|| {
                    fail_return(&format!("--from finding '{from}' not in frontier"))
                });
            if let LinkRef::Local { vf_id } = &parsed
                && !p.findings.iter().any(|f| &f.id == vf_id)
            {
                fail(&format!(
                    "local --to target '{vf_id}' not in frontier; add the target finding first"
                ));
            }
            if let LinkRef::Cross { vfr_id, .. } = &parsed
                && p.dep_for_vfr(vfr_id).is_none()
            {
                fail(&format!(
                    "cross-frontier --to references vfr_id '{vfr_id}' but no matching dep is declared. Run `vela frontier add-dep {vfr_id} --locator <url> --snapshot <hash>` first."
                ));
            }

            // v0.16: best-effort cross-frontier target-status check. The
            // substrate doctrine is "client verifies on read", but at
            // link-add time it's worth a one-shot fetch to warn the user
            // if their target has been superseded. Failure to fetch is
            // a hint, not a hard error — the link still records.
            let target_warning = if let LinkRef::Cross {
                vfr_id: target_vfr,
                vf_id: target_vf,
            } = &parsed
                && !no_check_target
                && let Some(dep) = p.dep_for_vfr(target_vfr)
                && let Some(locator) = dep.locator.as_deref()
                && (locator.starts_with("http://") || locator.starts_with("https://"))
            {
                cross_frontier_target_warning(locator, target_vfr, target_vf).await
            } else {
                None
            };

            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let link = Link {
                target: to.clone(),
                link_type: r#type.clone(),
                note: note.clone(),
                inferred_by: inferred_by.clone(),
                created_at: now,
                mechanism: None,
            };
            p.findings[source_idx].links.push(link);
            project::recompute_stats(&mut p);
            repo::save_to_path(&frontier, &p).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "link.add",
                "frontier": frontier.display().to_string(),
                "from": from,
                "to": to,
                "type": r#type,
                "cross_frontier": parsed.is_cross_frontier(),
            });
            if json {
                let mut p2 = payload.clone();
                if let Some(w) = &target_warning
                    && let serde_json::Value::Object(m) = &mut p2
                {
                    m.insert(
                        "target_warning".to_string(),
                        serde_json::Value::String(w.clone()),
                    );
                }
                print_json(&p2);
            } else {
                println!(
                    "{} {} --[{}]--> {}{}",
                    style::ok("link"),
                    from,
                    r#type,
                    to,
                    if parsed.is_cross_frontier() {
                        " (cross-frontier)"
                    } else {
                        ""
                    }
                );
                if let Some(w) = target_warning {
                    println!("  {w}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[tokio::test(flavor = "current_thread")]
    async fn cross_frontier_check_is_safe_inside_the_cli_runtime() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let remote = vela_protocol::project::assemble("remote", Vec::new(), 0, 0, "test");
        let body = serde_json::to_vec(&remote).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        });

        let warning = cross_frontier_target_warning(
            &format!("http://{address}/frontier.json"),
            "vfr_remote",
            "vf_missing",
        )
        .await;

        server.join().unwrap();
        assert!(
            warning.is_some_and(|message| message.contains("vf_missing")),
            "missing remote finding should produce a warning"
        );
    }
}
