//! Current-only `vela check`.

use std::path::{Path, PathBuf};

pub(crate) fn cmd_check(source: Option<&Path>, json_output: bool) {
    crate::ui::set_mode("check", json_output);
    let frontier = source.map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    if !frontier.is_dir() || !frontier.join(".vela/origin.json").is_file() {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Domain,
            "this Vela release verifies only current repository origins",
            Some(
                "inspect a predecessor with its pinned historical Vela release; current repositories contain `.vela/origin.json`",
            ),
        );
    }
    crate::current_repository::cmd_check_repository(&frontier, json_output);
}
