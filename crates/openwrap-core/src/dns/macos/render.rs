use std::path::Path;

use crate::dns::DnsPolicy;
use crate::profiles::ProfileId;

use super::paths::{shell_single_quote, shell_single_quote_str};

pub fn render_scoped_up_script(
    scoped_state_file: &Path,
    global_state_file: &Path,
    route_state_file: &Path,
    profile_id: &ProfileId,
) -> String {
    let template = include_str!("templates/scoped_up.sh");
    template
        .replace(
            "{{scoped_state_file}}",
            &shell_single_quote(scoped_state_file),
        )
        .replace(
            "{{global_state_file}}",
            &shell_single_quote(global_state_file),
        )
        .replace(
            "{{route_state_file}}",
            &shell_single_quote(route_state_file),
        )
        .replace(
            "{{profile_id}}",
            &shell_single_quote_str(&profile_id.to_string()),
        )
}

pub fn render_scoped_down_script(
    scoped_state_file: &Path,
    global_state_file: &Path,
    route_state_file: &Path,
    profile_id: &ProfileId,
) -> String {
    let template = include_str!("templates/scoped_down.sh");
    template
        .replace(
            "{{scoped_state_file}}",
            &shell_single_quote(scoped_state_file),
        )
        .replace(
            "{{global_state_file}}",
            &shell_single_quote(global_state_file),
        )
        .replace(
            "{{route_state_file}}",
            &shell_single_quote(route_state_file),
        )
        .replace(
            "{{profile_id}}",
            &shell_single_quote_str(&profile_id.to_string()),
        )
}

pub fn render_global_up_script(state_file: &Path, route_state_file: &Path) -> String {
    let template = include_str!("templates/global_up.sh");
    template
        .replace("{{state_file}}", &shell_single_quote(state_file))
        .replace(
            "{{route_state_file}}",
            &shell_single_quote(route_state_file),
        )
}

pub fn render_global_down_script(state_file: &Path, route_state_file: &Path) -> String {
    let template = include_str!("templates/global_down.sh");
    template
        .replace("{{state_file}}", &shell_single_quote(state_file))
        .replace(
            "{{route_state_file}}",
            &shell_single_quote(route_state_file),
        )
}

pub fn render_scripts(
    dns_policy: &DnsPolicy,
    scoped_state_file: &Path,
    global_state_file: &Path,
    route_state_file: &Path,
    profile_id: &ProfileId,
) -> (String, String) {
    match dns_policy {
        DnsPolicy::SplitDnsPreferred => (
            render_scoped_up_script(
                scoped_state_file,
                global_state_file,
                route_state_file,
                profile_id,
            ),
            render_scoped_down_script(
                scoped_state_file,
                global_state_file,
                route_state_file,
                profile_id,
            ),
        ),
        DnsPolicy::FullOverride => (
            render_global_up_script(global_state_file, route_state_file),
            render_global_down_script(global_state_file, route_state_file),
        ),
        DnsPolicy::ObserveOnly => unreachable!(),
    }
}
