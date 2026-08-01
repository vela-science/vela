use crate::cli::fail_return;
use std::path::Path;

pub(crate) fn cmd_status_compact(path: &Path, json_out: bool) {
    crate::current_repository::cmd_current_status(path, json_out);
}

pub(crate) fn cmd_log(
    path: &Path,
    object_id: Option<&str>,
    limit: usize,
    kind_filter: Option<&str>,
    as_of: Option<&str>,
    json: bool,
) {
    crate::ui::set_mode("log", json);
    let payload = crate::current_read::log_payload(path, object_id, limit, kind_filter, as_of)
        .unwrap_or_else(|error| fail_return(&error));
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize current log")
    );
}
