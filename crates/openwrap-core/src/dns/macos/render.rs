use std::path::Path;

use crate::profiles::ProfileId;

use super::paths;

const SCOPED_UP_TEMPLATE: &str = include_str!("templates/scoped_up.sh");
const SCOPED_DOWN_TEMPLATE: &str = include_str!("templates/scoped_down.sh");
const GLOBAL_UP_TEMPLATE: &str = include_str!("templates/global_up.sh");
const GLOBAL_DOWN_TEMPLATE: &str = include_str!("templates/global_down.sh");

pub fn render_scoped_up_script(
    scoped_state_file: &Path,
    global_state_file: &Path,
    route_state_file: &Path,
    profile_id: &ProfileId,
) -> String {
    SCOPED_UP_TEMPLATE
        .replace(
            "{scoped_state_file}",
            &paths::shell_single_quote(scoped_state_file),
        )
        .replace(
            "{global_state_file}",
            &paths::shell_single_quote(global_state_file),
        )
        .replace(
            "{route_state_file}",
            &paths::shell_single_quote(route_state_file),
        )
        .replace(
            "{profile_id}",
            &paths::shell_single_quote_str(&profile_id.to_string()),
        )
}

pub fn render_scoped_down_script(
    scoped_state_file: &Path,
    global_state_file: &Path,
    route_state_file: &Path,
    profile_id: &ProfileId,
) -> String {
    SCOPED_DOWN_TEMPLATE
        .replace(
            "{scoped_state_file}",
            &paths::shell_single_quote(scoped_state_file),
        )
        .replace(
            "{global_state_file}",
            &paths::shell_single_quote(global_state_file),
        )
        .replace(
            "{route_state_file}",
            &paths::shell_single_quote(route_state_file),
        )
        .replace(
            "{profile_id}",
            &paths::shell_single_quote_str(&profile_id.to_string()),
        )
}

pub fn render_global_up_script(state_file: &Path, route_state_file: &Path) -> String {
    GLOBAL_UP_TEMPLATE
        .replace("{state_file}", &paths::shell_single_quote(state_file))
        .replace(
            "{route_state_file}",
            &paths::shell_single_quote(route_state_file),
        )
}

pub fn render_global_down_script(state_file: &Path, route_state_file: &Path) -> String {
    GLOBAL_DOWN_TEMPLATE
        .replace("{state_file}", &paths::shell_single_quote(state_file))
        .replace(
            "{route_state_file}",
            &paths::shell_single_quote(route_state_file),
        )
}
