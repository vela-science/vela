use crate::cli::fail_return;
use std::path::Path;

pub(crate) fn cmd_status_compact(path: &Path, json_out: bool) {
    crate::repository_upgrade::cmd_current_status(path, json_out);
}

pub(crate) fn cmd_log(path: &Path, limit: usize, kind_filter: Option<&str>, _json: bool) {
    crate::ui::set_mode("log", true);
    let payload = crate::current_read::log_payload(path, limit, kind_filter)
        .unwrap_or_else(|error| fail_return(&error));
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize current log")
    );
}

pub(crate) async fn cmd_doctor(frontier: Option<&Path>, _port: u16, all: bool, json_output: bool) {
    let frontier = frontier.unwrap_or_else(|| Path::new("."));
    crate::current_doctor::cmd_current_doctor(frontier, all, json_output);
}
