use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::dns::DnsPolicy;
use crate::errors::AppError;
use crate::profiles::ProfileId;

const UP_SCRIPT_NAME: &str = "openwrap-dns-up.sh";
const DOWN_SCRIPT_NAME: &str = "openwrap-dns-down.sh";
const GLOBAL_STATE_NAME: &str = "global.tsv";
const SCOPED_STATE_NAME: &str = "scoped.tsv";

pub fn append_launch_config(
    config: &mut String,
    runtime_dir: &Path,
    profile_id: &ProfileId,
    dns_policy: &DnsPolicy,
) -> Result<Vec<PathBuf>, AppError> {
    if matches!(dns_policy, DnsPolicy::ObserveOnly) {
        return Ok(Vec::new());
    }

    let state_dir = persistent_state_dir(runtime_dir, profile_id)?;
    let bridge_dir = std::env::temp_dir()
        .join("openwrap-dns")
        .join(runtime_dir.file_name().unwrap_or_default());
    let up_script = bridge_dir.join(UP_SCRIPT_NAME);
    let down_script = bridge_dir.join(DOWN_SCRIPT_NAME);

    fs::create_dir_all(&state_dir)?;
    fs::create_dir_all(&bridge_dir)?;

    let (up_script_body, down_script_body) = match dns_policy {
        DnsPolicy::SplitDnsPreferred => (
            render_scoped_up_script(&state_dir.join(SCOPED_STATE_NAME), profile_id),
            render_scoped_down_script(&state_dir.join(SCOPED_STATE_NAME), profile_id),
        ),
        DnsPolicy::FullOverride => (
            render_global_up_script(&state_dir.join(GLOBAL_STATE_NAME)),
            render_global_down_script(&state_dir.join(GLOBAL_STATE_NAME)),
        ),
        DnsPolicy::ObserveOnly => unreachable!(),
    };

    fs::write(&up_script, up_script_body)?;
    fs::write(&down_script, down_script_body)?;
    make_executable(&up_script)?;
    make_executable(&down_script)?;

    config.push_str("script-security 2\n");
    config.push_str(&format!("up {}\n", quote_openvpn_arg(&up_script)));
    config.push_str(&format!("down {}\n", quote_openvpn_arg(&down_script)));
    config.push_str("down-pre\n");

    Ok(vec![bridge_dir])
}

fn render_scoped_up_script(state_file: &Path, profile_id: &ProfileId) -> String {
    format!(
        r##"#!/bin/sh
set -eu

STATE_FILE={state_file}
PROFILE_ID={profile_id}
RESOLVER_DIR=/etc/resolver
MARKER="# OpenWrap managed DNS"
PROFILE_MARKER="# profile_id=$PROFILE_ID"

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

collect_domains() {{
  foreign_vars=$(/usr/bin/env | /usr/bin/grep '^foreign_option_[0-9][0-9]*=' | /usr/bin/cut -d= -f1 | /usr/bin/sort -t_ -k3,3n || true)
  domains=""

  for var_name in $foreign_vars; do
    value=$(/usr/bin/printenv "$var_name" 2>/dev/null || true)
    case "$value" in
      "dhcp-option DOMAIN "*)
        domain_value=${{value#"dhcp-option DOMAIN "}}
        domains="$domains $domain_value"
        ;;
      "dhcp-option DOMAIN-SEARCH "*)
        domain_value=${{value#"dhcp-option DOMAIN-SEARCH "}}
        domains="$domains $domain_value"
        ;;
    esac
  done

  printf '%s' "$domains" | /usr/bin/tr '[:upper:]' '[:lower:]'
}}

normalize_domain() {{
  domain=$(printf '%s' "$1" | /usr/bin/sed 's/^[.]*//; s/[.]*$//')
  if [ -z "$domain" ]; then
    return 1
  fi

  case "$domain" in
    *[!A-Za-z0-9.-]*|*..*|/*|*/*|*\\*)
      return 1
      ;;
  esac

  printf '%s' "$domain"
}}

flush_dns_cache() {{
  /usr/bin/dscacheutil -flushcache >/dev/null 2>&1 || true
  /usr/bin/killall -HUP mDNSResponder >/dev/null 2>&1 || true
}}

dns_servers="$(collect_dns_servers)"
if [ -z "$dns_servers" ]; then
  /bin/rm -f "$STATE_FILE"
  exit 0
fi

domains_raw="$(collect_domains)"
if [ -z "$(printf '%s' "$domains_raw" | /usr/bin/tr -d '[:space:]')" ]; then
  /bin/rm -f "$STATE_FILE"
  echo "OPENWRAP_DNS_WARNING: VPN DNS servers were provided without VPN domains, so OpenWrap left normal system DNS unchanged. Switch this profile to FullOverride if all DNS should use the VPN."
  exit 0
fi

/bin/mkdir -p "$RESOLVER_DIR"
/bin/mkdir -p "$(/usr/bin/dirname "$STATE_FILE")"
tmp_file="${{STATE_FILE}}.tmp"
: > "$tmp_file"

for raw_domain in $domains_raw; do
  domain="$(normalize_domain "$raw_domain" || true)"
  [ -n "$domain" ] || continue
  resolver_path="$RESOLVER_DIR/$domain"

  if [ -f "$resolver_path" ]; then
    if ! /usr/bin/grep -q "^$MARKER$" "$resolver_path" || ! /usr/bin/grep -q "^$PROFILE_MARKER$" "$resolver_path"; then
      echo "OPENWRAP_DNS_WARNING: Skipped VPN DNS for domain '$domain' because /etc/resolver/$domain already exists and is not managed by this OpenWrap profile."
      continue
    fi
  fi

  tmp_resolver="${{resolver_path}}.openwrap.$$"
  {{
    printf '%s\n' "$MARKER"
    printf '%s\n' "$PROFILE_MARKER"
    printf '# session_id=%s\n' "$$"
    printf 'domain %s\n' "$domain"
    for dns_server in $dns_servers; do
      printf 'nameserver %s\n' "$dns_server"
    done
  }} > "$tmp_resolver"
  /bin/mv "$tmp_resolver" "$resolver_path"
  printf '%s\t%s\n' "$domain" "$resolver_path" >> "$tmp_file"
done

/bin/mv "$tmp_file" "$STATE_FILE"
flush_dns_cache
"##,
        state_file = shell_single_quote(state_file),
        profile_id = shell_single_quote_str(&profile_id.to_string()),
    )
}

fn render_scoped_down_script(state_file: &Path, profile_id: &ProfileId) -> String {
    format!(
        r##"#!/bin/sh
set -eu

STATE_FILE={state_file}
PROFILE_ID={profile_id}
MARKER="# OpenWrap managed DNS"
PROFILE_MARKER="# profile_id=$PROFILE_ID"

flush_dns_cache() {{
  /usr/bin/dscacheutil -flushcache >/dev/null 2>&1 || true
  /usr/bin/killall -HUP mDNSResponder >/dev/null 2>&1 || true
}}

[ -f "$STATE_FILE" ] || exit 0

tab="$(printf '\t')"
while IFS="$tab" read -r domain resolver_path; do
  [ -n "$resolver_path" ] || continue
  [ -f "$resolver_path" ] || continue

  if /usr/bin/grep -q "^$MARKER$" "$resolver_path" && /usr/bin/grep -q "^$PROFILE_MARKER$" "$resolver_path"; then
    /bin/rm -f "$resolver_path"
  fi
done < "$STATE_FILE"

/bin/rm -f "$STATE_FILE"
flush_dns_cache
"##,
        state_file = shell_single_quote(state_file),
        profile_id = shell_single_quote_str(&profile_id.to_string()),
    )
}

fn render_global_up_script(state_file: &Path) -> String {
    format!(
        r##"#!/bin/sh
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
if [ -z "$dns_servers" ]; then
  /bin/rm -f "$STATE_FILE"
  exit 0
fi

/bin/mkdir -p "$(/usr/bin/dirname "$STATE_FILE")"
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
"##,
        state_file = shell_single_quote(state_file),
    )
}

fn render_global_down_script(state_file: &Path) -> String {
    format!(
        r##"#!/bin/sh
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
"##,
        state_file = shell_single_quote(state_file),
    )
}

fn persistent_state_dir(runtime_dir: &Path, profile_id: &ProfileId) -> Result<PathBuf, AppError> {
    let runtime_root = runtime_dir
        .parent()
        .and_then(|parent| parent.parent())
        .ok_or_else(|| {
            AppError::ConnectionState("runtime directory is missing an expected root".into())
        })?;
    Ok(runtime_root.join("dns-state").join(profile_id.to_string()))
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

fn shell_single_quote_str(value: &str) -> String {
    let escaped = value.replace('\'', r#"'\''"#);
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

    use crate::dns::DnsPolicy;
    use crate::profiles::ProfileId;

    use super::append_launch_config;

    #[test]
    fn appends_scoped_runtime_dns_scripts_to_launch_config() {
        let temp = tempdir().unwrap();
        let runtime_dir = temp.path().join("runtime").join("profile").join("session");
        std::fs::create_dir_all(&runtime_dir).unwrap();
        let mut config = String::from("client\n");

        let cleanup_paths = append_launch_config(
            &mut config,
            &runtime_dir,
            &ProfileId::new(),
            &DnsPolicy::SplitDnsPreferred,
        )
        .unwrap();

        assert!(config.contains("script-security 2"));
        assert!(config.contains("openwrap-dns-up.sh"));
        assert!(config.contains("openwrap-dns-down.sh"));
        assert_eq!(cleanup_paths.len(), 1);
        assert!(cleanup_paths[0].join("openwrap-dns-up.sh").exists());
        assert!(cleanup_paths[0].join("openwrap-dns-down.sh").exists());
    }

    #[test]
    fn skips_script_injection_for_observe_only_profiles() {
        let temp = tempdir().unwrap();
        let runtime_dir = temp.path().join("runtime").join("profile").join("session");
        std::fs::create_dir_all(&runtime_dir).unwrap();
        let mut config = String::from("client\n");

        let cleanup_paths = append_launch_config(
            &mut config,
            &runtime_dir,
            &ProfileId::new(),
            &DnsPolicy::ObserveOnly,
        )
        .unwrap();

        assert_eq!(config, "client\n");
        assert!(cleanup_paths.is_empty());
    }
}
