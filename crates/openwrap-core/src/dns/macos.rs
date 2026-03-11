use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::errors::AppError;

const UP_SCRIPT_NAME: &str = "openwrap-dns-up.sh";
const DOWN_SCRIPT_NAME: &str = "openwrap-dns-down.sh";
const DNS_STATE_NAME: &str = "openwrap-dns-state.tsv";

pub fn append_launch_config(
    config: &mut String,
    runtime_dir: &Path,
) -> Result<Vec<std::path::PathBuf>, AppError> {
    let state_file = runtime_dir.join(DNS_STATE_NAME);
    let bridge_dir = std::env::temp_dir()
        .join("openwrap-dns")
        .join(runtime_dir.file_name().unwrap_or_default());
    let up_script = bridge_dir.join(UP_SCRIPT_NAME);
    let down_script = bridge_dir.join(DOWN_SCRIPT_NAME);

    fs::create_dir_all(&bridge_dir)?;
    fs::write(&up_script, render_up_script(&state_file))?;
    fs::write(&down_script, render_down_script(&state_file))?;
    make_executable(&up_script)?;
    make_executable(&down_script)?;

    config.push_str("script-security 2\n");
    config.push_str(&format!("up {}\n", quote_openvpn_arg(&up_script)));
    config.push_str(&format!("down {}\n", quote_openvpn_arg(&down_script)));
    config.push_str("down-pre\n");

    Ok(vec![bridge_dir])
}

fn render_up_script(state_file: &Path) -> String {
    format!(
        r#"#!/bin/sh
set -eu

STATE_FILE={state_file}
NETWORKSETUP=/usr/sbin/networksetup

collect_dns_servers() {{
  foreign_vars=$(/usr/bin/env | /usr/bin/grep '^foreign_option_[0-9][0-9]*=' | /usr/bin/cut -d= -f1 | /usr/bin/sort -t_ -k3,3n || true)
  dns_servers=""

  for var_name in $foreign_vars; do
    value=$(/usr/bin/printenv "$var_name" 2>/dev/null || true)
    case "$value" in
      "dhcp-option DNS "*)
        dns_value=${{value#"dhcp-option DNS "}}
        dns_value=${{dns_value%% *}}
        if [ -n "$dns_value" ]; then
          if [ -z "$dns_servers" ]; then
            dns_servers="$dns_value"
          else
            dns_servers="$dns_servers $dns_value"
          fi
        fi
        ;;
    esac
  done

  printf '%s' "$dns_servers"
}}

flush_dns_cache() {{
  /usr/bin/dscacheutil -flushcache >/dev/null 2>&1 || true
  /usr/bin/killall -HUP mDNSResponder >/dev/null 2>&1 || true
}}

dns_servers="$(collect_dns_servers)"
[ -n "$dns_servers" ] || exit 0

tmp_file="${{STATE_FILE}}.tmp"
: > "$tmp_file"

"$NETWORKSETUP" -listallnetworkservices | while IFS= read -r service; do
  [ "$service" = "An asterisk (*) denotes that a network service is disabled." ] && continue
  [ -n "$service" ] || continue
  case "$service" in
    \**) continue ;;
  esac

  current_dns=$("$NETWORKSETUP" -getdnsservers "$service" 2>/dev/null || true)
  if printf '%s\n' "$current_dns" | /usr/bin/grep -q "There aren't any DNS Servers set on"; then
    current_dns="__EMPTY__"
  else
    current_dns=$(printf '%s\n' "$current_dns" | /usr/bin/tr '\n' ' ' | /usr/bin/sed 's/[[:space:]]*$//')
    [ -n "$current_dns" ] || current_dns="__EMPTY__"
  fi

  printf '%s\t%s\n' "$service" "$current_dns" >> "$tmp_file"
  # shellcheck disable=SC2086
  "$NETWORKSETUP" -setdnsservers "$service" $dns_servers >/dev/null 2>&1 || true
done

/bin/mv "$tmp_file" "$STATE_FILE"
flush_dns_cache
"#,
        state_file = shell_single_quote(state_file),
    )
}

fn render_down_script(state_file: &Path) -> String {
    format!(
        r#"#!/bin/sh
set -eu

STATE_FILE={state_file}
NETWORKSETUP=/usr/sbin/networksetup

flush_dns_cache() {{
  /usr/bin/dscacheutil -flushcache >/dev/null 2>&1 || true
  /usr/bin/killall -HUP mDNSResponder >/dev/null 2>&1 || true
}}

[ -f "$STATE_FILE" ] || exit 0

tab="$(printf '\t')"
while IFS="$tab" read -r service current_dns; do
  [ -n "$service" ] || continue

  if [ "$current_dns" = "__EMPTY__" ]; then
    "$NETWORKSETUP" -setdnsservers "$service" Empty >/dev/null 2>&1 || true
    continue
  fi

  set -- $current_dns
  "$NETWORKSETUP" -setdnsservers "$service" "$@" >/dev/null 2>&1 || true
done < "$STATE_FILE"

/bin/rm -f "$STATE_FILE"
flush_dns_cache
"#,
        state_file = shell_single_quote(state_file),
    )
}

fn quote_openvpn_arg(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace(' ', "\\ ")
}

fn shell_single_quote(path: &Path) -> String {
    let escaped = path.to_string_lossy().replace('\'', r#"'\''"#);
    format!("'{escaped}'")
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), AppError> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::append_launch_config;

    #[test]
    fn appends_runtime_dns_scripts_to_launch_config() {
        let temp = tempdir().unwrap();
        let mut config = String::from("client\n");

        let cleanup_paths = append_launch_config(&mut config, temp.path()).unwrap();

        assert!(config.contains("script-security 2"));
        assert!(config.contains("openwrap-dns-up.sh"));
        assert!(config.contains("openwrap-dns-down.sh"));
        assert_eq!(cleanup_paths.len(), 1);
        assert!(cleanup_paths[0].join("openwrap-dns-up.sh").exists());
        assert!(cleanup_paths[0].join("openwrap-dns-down.sh").exists());
    }
}
