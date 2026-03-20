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
const ROUTE_STATE_NAME: &str = "dns-routes.tsv";

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

    let scoped_state = state_dir.join(SCOPED_STATE_NAME);
    let global_state = state_dir.join(GLOBAL_STATE_NAME);
    let route_state = state_dir.join(ROUTE_STATE_NAME);

    let (up_script_body, down_script_body) = match dns_policy {
        DnsPolicy::SplitDnsPreferred => (
            render_scoped_up_script(&scoped_state, &global_state, &route_state, profile_id),
            render_scoped_down_script(&scoped_state, &global_state, &route_state, profile_id),
        ),
        DnsPolicy::FullOverride => (
            render_global_up_script(&global_state, &route_state),
            render_global_down_script(&global_state, &route_state),
        ),
        DnsPolicy::ObserveOnly => unreachable!(),
    };

    fs::write(&up_script, up_script_body)?;
    fs::write(&down_script, down_script_body)?;
    make_executable(&up_script)?;
    make_executable(&down_script)?;

    config.push_str("script-security 2\n");
    config.push_str(&format!("route-up {}\n", quote_openvpn_arg(&up_script)));
    config.push_str(&format!(
        "route-pre-down {}\n",
        quote_openvpn_arg(&down_script)
    ));
    config.push_str(&format!("down {}\n", quote_openvpn_arg(&down_script)));
    config.push_str("down-pre\n");

    Ok(vec![bridge_dir])
}

fn render_scoped_up_script(
    scoped_state_file: &Path,
    global_state_file: &Path,
    route_state_file: &Path,
    profile_id: &ProfileId,
) -> String {
    format!(
        r##"#!/bin/sh
set -eu

SCOPED_STATE_FILE={scoped_state_file}
GLOBAL_STATE_FILE={global_state_file}
ROUTE_STATE_FILE={route_state_file}
PROFILE_ID={profile_id}
RESOLVER_DIR=/etc/resolver
NETWORKSETUP=/usr/sbin/networksetup
SCUTIL=/usr/sbin/scutil
ROUTE=/sbin/route
MARKER="# OpenWrap managed DNS"
PROFILE_MARKER="# profile_id=$PROFILE_ID"
DEV="${{dev:-${{1:-}}}}"
VPN_GATEWAY="${{route_vpn_gateway:-}}"

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

collect_match_domains() {{
  foreign_vars=$(/usr/bin/env | /usr/bin/grep '^foreign_option_[0-9][0-9]*=' | /usr/bin/cut -d= -f1 | /usr/bin/sort -t_ -k3,3n || true)

  for var_name in $foreign_vars; do
    value=$(/usr/bin/printenv "$var_name" 2>/dev/null || true)
    case "$value" in
      "dhcp-option DOMAIN "*)
        domain_value=${{value#"dhcp-option DOMAIN "}}
        [ -n "$domain_value" ] && printf '%s\n' "$domain_value"
        ;;
    esac
  done | /usr/bin/tr '[:upper:]' '[:lower:]'
}}

collect_search_domains() {{
  foreign_vars=$(/usr/bin/env | /usr/bin/grep '^foreign_option_[0-9][0-9]*=' | /usr/bin/cut -d= -f1 | /usr/bin/sort -t_ -k3,3n || true)

  for var_name in $foreign_vars; do
    value=$(/usr/bin/printenv "$var_name" 2>/dev/null || true)
    case "$value" in
      "dhcp-option DOMAIN-SEARCH "*)
        search_values=${{value#"dhcp-option DOMAIN-SEARCH "}}
        for domain_value in $search_values; do
          [ -n "$domain_value" ] && printf '%s\n' "$domain_value"
        done
        ;;
    esac
  done | /usr/bin/tr '[:upper:]' '[:lower:]'
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

log_debug() {{
  printf '%s\n' "OPENWRAP_DNS_DEBUG: $*" >&2
}}

log_error() {{
  printf '%s\n' "OPENWRAP_DNS_ERROR: $*" >&2
}}

read_service_dns() {{
  service="$1"
  current_dns=$("$NETWORKSETUP" -getdnsservers "$service" 2>/dev/null || true)
  if printf '%s\n' "$current_dns" | /usr/bin/grep -q "There aren't any DNS Servers set on"; then
    printf '__EMPTY__'
  else
    current_dns=$(printf '%s\n' "$current_dns" | /usr/bin/tr '\n' ' ' | /usr/bin/sed 's/[[:space:]]*$//')
    [ -n "$current_dns" ] || current_dns="__EMPTY__"
    printf '%s' "$current_dns"
  fi
}}

verify_service_dns() {{
  service="$1"
  expected="$2"
  actual="$(read_service_dns "$service")"
  [ "$actual" = "$expected" ]
}}

restore_service_dns() {{
  service="$1"
  current_dns="$2"

  if [ "$current_dns" = "__EMPTY__" ]; then
    "$NETWORKSETUP" -setdnsservers "$service" Empty >/dev/null 2>&1 || return 1
  else
    set -- $current_dns
    "$NETWORKSETUP" -setdnsservers "$service" "$@" >/dev/null 2>&1 || return 1
  fi

  verify_service_dns "$service" "$current_dns"
}}

list_target_services() {{
  service_order_file="${{GLOBAL_STATE_FILE}}.services.$$"
  active_device_file="${{GLOBAL_STATE_FILE}}.devices.$$"
  "$SCUTIL" --nwi 2>/dev/null | /usr/bin/awk '/^[[:space:]]*[[:alnum:]][[:alnum:]]*[[:space:]]*:/ {{ device=$1; gsub(":", "", device); print device }}' > "$active_device_file"
  active_devices="$(/usr/bin/tr '\n' ' ' < "$active_device_file" | /usr/bin/sed 's/[[:space:]]*$//')"
  [ -n "$active_devices" ] && log_debug "active network devices: $active_devices"
  "$NETWORKSETUP" -listnetworkserviceorder > "$service_order_file" 2>/dev/null || {{
    /bin/rm -f "$service_order_file" "$active_device_file"
    return 1
  }}

  current_service=""
  while IFS= read -r line; do
    case "$line" in
      \(*\)\ *)
        current_service=${{line#*) }}
        ;;
      "(Hardware Port:"*)
        device=$(printf '%s\n' "$line" | /usr/bin/sed -n 's/.*Device: \([^)]*\)).*/\1/p')
        if [ -n "$current_service" ] && [ -n "$device" ] && /usr/bin/grep -Fxq "$device" "$active_device_file"; then
          log_debug "selected active service '$current_service' on device '$device'"
          printf '%s\n' "$current_service"
        fi
        current_service=""
        ;;
    esac
  done < "$service_order_file"

  /bin/rm -f "$service_order_file" "$active_device_file"
}}

route_uses_vpn() {{
  destination="$1"
  route_output=$("$ROUTE" -n get "$destination" 2>/dev/null || true)
  if [ -n "$DEV" ] && printf '%s\n' "$route_output" | /usr/bin/grep -Eq "interface: $DEV$"; then
    return 0
  fi
  if [ -n "$VPN_GATEWAY" ] && printf '%s\n' "$route_output" | /usr/bin/grep -Eq "gateway: $VPN_GATEWAY$"; then
    return 0
  fi
  return 1
}}

ensure_dns_servers_routable() {{
  dns_list="$1"

  for dns_server in $dns_list; do
    if route_uses_vpn "$dns_server"; then
      log_debug "verified VPN route to DNS server '$dns_server'"
      continue
    fi
    route_output=$("$ROUTE" -n get "$dns_server" 2>/dev/null || true)
    [ -n "$route_output" ] && log_error "DNS server '$dns_server' is not routed through the VPN: $(printf '%s' "$route_output" | /usr/bin/tr '\n' ' ' | /usr/bin/sed 's/[[:space:]]*$//')"
    printf '%s\n' "OPENWRAP_DNS_WARNING: VPN_DNS_NOT_ROUTED" >&2
    return 1
  done
}}

rollback_global_state_preserve() {{
  state_file="$1"
  [ -f "$state_file" ] || return 0

  failed=0
  tab="$(printf '\t')"
  while IFS="$tab" read -r service current_dns; do
    [ -n "$service" ] || continue
    restore_service_dns "$service" "$current_dns" || failed=1
  done < "$state_file"

  flush_dns_cache
  [ "$failed" -eq 0 ]
}}

apply_global_override() {{
  state_file="$1"
  route_state_file="$2"
  desired_dns="$3"

  ensure_dns_servers_routable "$desired_dns" || return 1
  /bin/mkdir -p "$(/usr/bin/dirname "$state_file")"
  tmp_file="${{state_file}}.tmp"
  services_file="${{state_file}}.targets.$$"
  cleanup_tmp_files() {{
    /bin/rm -f "$services_file"
  }}
  trap cleanup_tmp_files EXIT INT TERM
  : > "$tmp_file"

  list_target_services > "$services_file" || {{
    log_error "failed to enumerate active network services"
    /bin/rm -f "$tmp_file"
    return 1
  }}
  [ -s "$services_file" ] || {{
    log_error "no active network services available for DNS override"
    /bin/rm -f "$tmp_file"
    return 1
  }}

  apply_failed=0
  while IFS= read -r service; do
    [ -n "$service" ] || continue

    current_dns="$(read_service_dns "$service")"
    if [ "$current_dns" = "$desired_dns" ]; then
      log_debug "service '$service' already uses VPN DNS '$desired_dns'"
      continue
    fi

    log_debug "applying VPN DNS '$desired_dns' to service '$service' (current='$current_dns')"
    printf '%s\t%s\n' "$service" "$current_dns" >> "$tmp_file"
    set -- $desired_dns
    "$NETWORKSETUP" -setdnsservers "$service" "$@" >/dev/null 2>&1 || {{
      log_error "networksetup failed while applying VPN DNS to service '$service'"
      apply_failed=1
      break
    }}
    verify_service_dns "$service" "$desired_dns" || {{
      actual_dns="$(read_service_dns "$service" || true)"
      log_error "DNS verification failed for service '$service' (expected='$desired_dns' actual='${{actual_dns:-unknown}}')"
      apply_failed=1
      break
    }}
  done < "$services_file"

  if [ "$apply_failed" -ne 0 ]; then
    if [ -s "$tmp_file" ]; then
      /bin/mv "$tmp_file" "$state_file"
    else
      /bin/rm -f "$tmp_file"
    fi
    rollback_global_state_preserve "$state_file" || true
    flush_dns_cache
    return 1
  fi

  if [ -s "$tmp_file" ]; then
    /bin/mv "$tmp_file" "$state_file"
  else
    /bin/rm -f "$tmp_file" "$state_file"
  fi

  flush_dns_cache
  trap - EXIT INT TERM
  cleanup_tmp_files
  return 0
}}

write_resolver() {{
  domain="$1"
  resolver_mode="$2"
  resolver_path="$RESOLVER_DIR/$domain"

  if [ -f "$resolver_path" ]; then
    if ! /usr/bin/grep -q "^$MARKER$" "$resolver_path" || ! /usr/bin/grep -q "^$PROFILE_MARKER$" "$resolver_path"; then
      printf '%s\n' "OPENWRAP_DNS_WARNING: Skipped VPN DNS for domain '$domain' because /etc/resolver/$domain already exists and is not managed by this OpenWrap profile." >&2
      return
    fi
  fi

  tmp_resolver="${{resolver_path}}.openwrap.$$"
  {{
    printf '%s\n' "$MARKER"
    printf '%s\n' "$PROFILE_MARKER"
    printf '# session_id=%s\n' "$$"
    if [ "$resolver_mode" = "search" ]; then
      printf 'search %s\n' "$domain"
    else
      printf 'domain %s\n' "$domain"
    fi
    for dns_server in $dns_servers; do
      printf 'nameserver %s\n' "$dns_server"
    done
  }} > "$tmp_resolver"
  /bin/mv "$tmp_resolver" "$resolver_path"
  printf '%s\t%s\n' "$domain" "$resolver_path" >> "$tmp_file"
  log_debug "wrote scoped resolver '$resolver_path' for domain '$domain' using mode '$resolver_mode'"
}}

dns_servers="$(collect_dns_servers)"
if [ -z "$dns_servers" ]; then
  /bin/rm -f "$SCOPED_STATE_FILE" "$GLOBAL_STATE_FILE" "$ROUTE_STATE_FILE"
  exit 0
fi
log_debug "observed VPN DNS servers: $dns_servers"

match_domains="$(collect_match_domains)"
search_domains="$(collect_search_domains)"
log_debug "observed VPN match domains: $match_domains"
log_debug "observed VPN search domains: $search_domains"
if [ -z "$(printf '%s%s' "$match_domains" "$search_domains" | /usr/bin/tr -d '[:space:]')" ]; then
  /bin/rm -f "$SCOPED_STATE_FILE"
  log_debug "VPN pushed DNS servers without domains; auto-promoting to full override"
  if ! apply_global_override "$GLOBAL_STATE_FILE" "$ROUTE_STATE_FILE" "$dns_servers"; then
    exit 1
  fi
  printf '%s\n' "OPENWRAP_DNS_WARNING: AUTO_PROMOTED_FULL_OVERRIDE" >&2
  exit 0
fi

/bin/rm -f "$GLOBAL_STATE_FILE" "$ROUTE_STATE_FILE"
/bin/mkdir -p "$RESOLVER_DIR"
/bin/mkdir -p "$(/usr/bin/dirname "$SCOPED_STATE_FILE")"
tmp_file="${{SCOPED_STATE_FILE}}.tmp"
: > "$tmp_file"

for raw_domain in $match_domains; do
  domain="$(normalize_domain "$raw_domain" || true)"
  [ -n "$domain" ] || continue
  write_resolver "$domain" "domain"
done

for raw_domain in $search_domains; do
  domain="$(normalize_domain "$raw_domain" || true)"
  [ -n "$domain" ] || continue
  write_resolver "$domain" "search"
done

/bin/mv "$tmp_file" "$SCOPED_STATE_FILE"
log_debug "scoped DNS resolver installation completed"
flush_dns_cache
"##,
        scoped_state_file = shell_single_quote(scoped_state_file),
        global_state_file = shell_single_quote(global_state_file),
        route_state_file = shell_single_quote(route_state_file),
        profile_id = shell_single_quote_str(&profile_id.to_string()),
    )
}

fn render_scoped_down_script(
    scoped_state_file: &Path,
    global_state_file: &Path,
    route_state_file: &Path,
    profile_id: &ProfileId,
) -> String {
    format!(
        r##"#!/bin/sh
set -eu

SCOPED_STATE_FILE={scoped_state_file}
GLOBAL_STATE_FILE={global_state_file}
ROUTE_STATE_FILE={route_state_file}
PROFILE_ID={profile_id}
NETWORKSETUP=/usr/sbin/networksetup
ROUTE=/sbin/route
MARKER="# OpenWrap managed DNS"
PROFILE_MARKER="# profile_id=$PROFILE_ID"

flush_dns_cache() {{
  /usr/bin/dscacheutil -flushcache >/dev/null 2>&1 || true
  /usr/bin/killall -HUP mDNSResponder >/dev/null 2>&1 || true
}}

log_debug() {{
  printf '%s\n' "OPENWRAP_DNS_DEBUG: $*" >&2
}}

log_error() {{
  printf '%s\n' "OPENWRAP_DNS_ERROR: $*" >&2
}}

read_service_dns() {{
  service="$1"
  current_dns=$("$NETWORKSETUP" -getdnsservers "$service" 2>/dev/null || true)
  if printf '%s\n' "$current_dns" | /usr/bin/grep -q "There aren't any DNS Servers set on"; then
    printf '__EMPTY__'
  else
    current_dns=$(printf '%s\n' "$current_dns" | /usr/bin/tr '\n' ' ' | /usr/bin/sed 's/[[:space:]]*$//')
    [ -n "$current_dns" ] || current_dns="__EMPTY__"
    printf '%s' "$current_dns"
  fi
}}

verify_service_dns() {{
  service="$1"
  expected="$2"
  actual="$(read_service_dns "$service")"
  [ "$actual" = "$expected" ]
}}

restore_service_dns() {{
  service="$1"
  current_dns="$2"

  if [ "$current_dns" = "__EMPTY__" ]; then
    "$NETWORKSETUP" -setdnsservers "$service" Empty >/dev/null 2>&1 || return 1
  else
    set -- $current_dns
    "$NETWORKSETUP" -setdnsservers "$service" "$@" >/dev/null 2>&1 || return 1
  fi

  verify_service_dns "$service" "$current_dns"
}}

remove_recorded_dns_routes() {{
  state_file="$1"
  [ -f "$state_file" ] || return 0

  failed=0
  tab="$(printf '\t')"
  while IFS="$tab" read -r dns_server dns_gateway; do
    [ -n "$dns_server" ] || continue
    "$ROUTE" -n delete -host "$dns_server" "${{dns_gateway:-}}" >/dev/null 2>&1 || "$ROUTE" -n delete -host "$dns_server" >/dev/null 2>&1 || failed=1
  done < "$state_file"

  if [ "$failed" -eq 0 ]; then
    /bin/rm -f "$state_file"
    return 0
  fi

  return 1
}}

restore_global_state() {{
  state_file="$1"
  [ -f "$state_file" ] || return 0

  failed=0
  tab="$(printf '\t')"
  while IFS="$tab" read -r service current_dns; do
    [ -n "$service" ] || continue
    restore_service_dns "$service" "$current_dns" || failed=1
  done < "$state_file"

  if [ "$failed" -eq 0 ]; then
    /bin/rm -f "$state_file"
    return 0
  fi

  printf '%s\n' "OPENWRAP_DNS_WARNING: RESTORE_PENDING_RECONCILE" >&2
  return 1
}}

remove_scoped_resolvers() {{
  state_file="$1"
  [ -f "$state_file" ] || return 0

  failed=0
  tab="$(printf '\t')"
  while IFS="$tab" read -r domain resolver_path; do
    [ -n "$resolver_path" ] || continue
    [ -f "$resolver_path" ] || continue

    if /usr/bin/grep -q "^$MARKER$" "$resolver_path" && /usr/bin/grep -q "^$PROFILE_MARKER$" "$resolver_path"; then
      /bin/rm -f "$resolver_path" || failed=1
    else
      failed=1
    fi
  done < "$state_file"

  if [ "$failed" -eq 0 ]; then
    /bin/rm -f "$state_file"
    return 0
  fi

  printf '%s\n' "OPENWRAP_DNS_WARNING: RESTORE_FAILED" >&2
  return 1
}}

failed=0
remove_scoped_resolvers "$SCOPED_STATE_FILE" || failed=1
remove_recorded_dns_routes "$ROUTE_STATE_FILE" || failed=1
restore_global_state "$GLOBAL_STATE_FILE" || failed=1
flush_dns_cache
[ "$failed" -eq 0 ]
"##,
        scoped_state_file = shell_single_quote(scoped_state_file),
        global_state_file = shell_single_quote(global_state_file),
        route_state_file = shell_single_quote(route_state_file),
        profile_id = shell_single_quote_str(&profile_id.to_string()),
    )
}

fn render_global_up_script(state_file: &Path, route_state_file: &Path) -> String {
    format!(
        r##"#!/bin/sh
set -eu

STATE_FILE={state_file}
ROUTE_STATE_FILE={route_state_file}
NETWORKSETUP=/usr/sbin/networksetup
SCUTIL=/usr/sbin/scutil
ROUTE=/sbin/route
DEV="${{dev:-${{1:-}}}}"
VPN_GATEWAY="${{route_vpn_gateway:-}}"

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

log_debug() {{
  printf '%s\n' "OPENWRAP_DNS_DEBUG: $*" >&2
}}

log_error() {{
  printf '%s\n' "OPENWRAP_DNS_ERROR: $*" >&2
}}

read_service_dns() {{
  service="$1"
  current_dns=$("$NETWORKSETUP" -getdnsservers "$service" 2>/dev/null || true)
  if printf '%s\n' "$current_dns" | /usr/bin/grep -q "There aren't any DNS Servers set on"; then
    printf '__EMPTY__'
  else
    current_dns=$(printf '%s\n' "$current_dns" | /usr/bin/tr '\n' ' ' | /usr/bin/sed 's/[[:space:]]*$//')
    [ -n "$current_dns" ] || current_dns="__EMPTY__"
    printf '%s' "$current_dns"
  fi
}}

verify_service_dns() {{
  service="$1"
  expected="$2"
  actual="$(read_service_dns "$service")"
  [ "$actual" = "$expected" ]
}}

restore_service_dns() {{
  service="$1"
  current_dns="$2"

  if [ "$current_dns" = "__EMPTY__" ]; then
    "$NETWORKSETUP" -setdnsservers "$service" Empty >/dev/null 2>&1 || return 1
  else
    set -- $current_dns
    "$NETWORKSETUP" -setdnsservers "$service" "$@" >/dev/null 2>&1 || return 1
  fi

  verify_service_dns "$service" "$current_dns"
}}

list_target_services() {{
  service_order_file="${{STATE_FILE}}.services.$$"
  active_device_file="${{STATE_FILE}}.devices.$$"
  "$SCUTIL" --nwi 2>/dev/null | /usr/bin/awk '/^[[:space:]]*[[:alnum:]][[:alnum:]]*[[:space:]]*:/ {{ device=$1; gsub(":", "", device); print device }}' > "$active_device_file"
  active_devices="$(/usr/bin/tr '\n' ' ' < "$active_device_file" | /usr/bin/sed 's/[[:space:]]*$//')"
  [ -n "$active_devices" ] && log_debug "active network devices: $active_devices"
  "$NETWORKSETUP" -listnetworkserviceorder > "$service_order_file" 2>/dev/null || {{
    /bin/rm -f "$service_order_file" "$active_device_file"
    return 1
  }}

  current_service=""
  while IFS= read -r line; do
    case "$line" in
      \(*\)\ *)
        current_service=${{line#*) }}
        ;;
      "(Hardware Port:"*)
        device=$(printf '%s\n' "$line" | /usr/bin/sed -n 's/.*Device: \([^)]*\)).*/\1/p')
        if [ -n "$current_service" ] && [ -n "$device" ] && /usr/bin/grep -Fxq "$device" "$active_device_file"; then
          log_debug "selected active service '$current_service' on device '$device'"
          printf '%s\n' "$current_service"
        fi
        current_service=""
        ;;
    esac
  done < "$service_order_file"

  /bin/rm -f "$service_order_file" "$active_device_file"
}}

route_uses_vpn() {{
  destination="$1"
  route_output=$("$ROUTE" -n get "$destination" 2>/dev/null || true)
  if [ -n "$DEV" ] && printf '%s\n' "$route_output" | /usr/bin/grep -Eq "interface: $DEV$"; then
    return 0
  fi
  if [ -n "$VPN_GATEWAY" ] && printf '%s\n' "$route_output" | /usr/bin/grep -Eq "gateway: $VPN_GATEWAY$"; then
    return 0
  fi
  return 1
}}

ensure_dns_servers_routable() {{
  dns_list="$1"

  for dns_server in $dns_list; do
    if route_uses_vpn "$dns_server"; then
      log_debug "verified VPN route to DNS server '$dns_server'"
      continue
    fi
    route_output=$("$ROUTE" -n get "$dns_server" 2>/dev/null || true)
    [ -n "$route_output" ] && log_error "DNS server '$dns_server' is not routed through the VPN: $(printf '%s' "$route_output" | /usr/bin/tr '\n' ' ' | /usr/bin/sed 's/[[:space:]]*$//')"
    printf '%s\n' "OPENWRAP_DNS_WARNING: VPN_DNS_NOT_ROUTED" >&2
    return 1
  done
}}

rollback_global_state_preserve() {{
  state_file="$1"
  [ -f "$state_file" ] || return 0

  failed=0
  tab="$(printf '\t')"
  while IFS="$tab" read -r service current_dns; do
    [ -n "$service" ] || continue
    restore_service_dns "$service" "$current_dns" || failed=1
  done < "$state_file"

  flush_dns_cache
  [ "$failed" -eq 0 ]
}}

dns_servers="$(collect_dns_servers)"
if [ -z "$dns_servers" ]; then
  /bin/rm -f "$STATE_FILE" "$ROUTE_STATE_FILE"
  exit 0
fi
log_debug "observed VPN DNS servers: $dns_servers"

ensure_dns_servers_routable "$dns_servers" || exit 1
/bin/mkdir -p "$(/usr/bin/dirname "$STATE_FILE")"
tmp_file="${{STATE_FILE}}.tmp"
services_file="${{STATE_FILE}}.targets.$$"
cleanup_tmp_files() {{
  /bin/rm -f "$services_file"
}}
trap cleanup_tmp_files EXIT INT TERM
: > "$tmp_file"

list_target_services > "$services_file" || {{
  log_error "failed to enumerate active network services"
  /bin/rm -f "$tmp_file"
  exit 1
}}
[ -s "$services_file" ] || {{
  log_error "no active network services available for DNS override"
  /bin/rm -f "$tmp_file"
  exit 1
}}

apply_failed=0
while IFS= read -r service; do
  [ -n "$service" ] || continue

  current_dns="$(read_service_dns "$service")"
  if [ "$current_dns" = "$dns_servers" ]; then
    log_debug "service '$service' already uses VPN DNS '$dns_servers'"
    continue
  fi

  log_debug "applying VPN DNS '$dns_servers' to service '$service' (current='$current_dns')"
  printf '%s\t%s\n' "$service" "$current_dns" >> "$tmp_file"
  set -- $dns_servers
  "$NETWORKSETUP" -setdnsservers "$service" "$@" >/dev/null 2>&1 || {{
    log_error "networksetup failed while applying VPN DNS to service '$service'"
    apply_failed=1
    break
  }}
  verify_service_dns "$service" "$dns_servers" || {{
    actual_dns="$(read_service_dns "$service" || true)"
    log_error "DNS verification failed for service '$service' (expected='$dns_servers' actual='${{actual_dns:-unknown}}')"
    apply_failed=1
    break
  }}
done < "$services_file"

if [ "$apply_failed" -ne 0 ]; then
  if [ -s "$tmp_file" ]; then
    /bin/mv "$tmp_file" "$STATE_FILE"
  else
    /bin/rm -f "$tmp_file"
  fi
  rollback_global_state_preserve "$STATE_FILE" || true
  flush_dns_cache
  exit 1
fi

if [ -s "$tmp_file" ]; then
  /bin/mv "$tmp_file" "$STATE_FILE"
else
  /bin/rm -f "$tmp_file" "$STATE_FILE"
fi

log_debug "global DNS override applied successfully"
flush_dns_cache
trap - EXIT INT TERM
cleanup_tmp_files
"##,
        state_file = shell_single_quote(state_file),
        route_state_file = shell_single_quote(route_state_file),
    )
}

fn render_global_down_script(state_file: &Path, route_state_file: &Path) -> String {
    format!(
        r##"#!/bin/sh
set -eu

STATE_FILE={state_file}
ROUTE_STATE_FILE={route_state_file}
NETWORKSETUP=/usr/sbin/networksetup
ROUTE=/sbin/route

flush_dns_cache() {{
  /usr/bin/dscacheutil -flushcache >/dev/null 2>&1 || true
  /usr/bin/killall -HUP mDNSResponder >/dev/null 2>&1 || true
}}

read_service_dns() {{
  service="$1"
  current_dns=$("$NETWORKSETUP" -getdnsservers "$service" 2>/dev/null || true)
  if printf '%s\n' "$current_dns" | /usr/bin/grep -q "There aren't any DNS Servers set on"; then
    printf '__EMPTY__'
  else
    current_dns=$(printf '%s\n' "$current_dns" | /usr/bin/tr '\n' ' ' | /usr/bin/sed 's/[[:space:]]*$//')
    [ -n "$current_dns" ] || current_dns="__EMPTY__"
    printf '%s' "$current_dns"
  fi
}}

verify_service_dns() {{
  service="$1"
  expected="$2"
  actual="$(read_service_dns "$service")"
  [ "$actual" = "$expected" ]
}}

restore_service_dns() {{
  service="$1"
  current_dns="$2"

  if [ "$current_dns" = "__EMPTY__" ]; then
    "$NETWORKSETUP" -setdnsservers "$service" Empty >/dev/null 2>&1 || return 1
  else
    set -- $current_dns
    "$NETWORKSETUP" -setdnsservers "$service" "$@" >/dev/null 2>&1 || return 1
  fi

  verify_service_dns "$service" "$current_dns"
}}

remove_recorded_dns_routes() {{
  state_file="$1"
  [ -f "$state_file" ] || return 0

  failed=0
  tab="$(printf '\t')"
  while IFS="$tab" read -r dns_server dns_gateway; do
    [ -n "$dns_server" ] || continue
    "$ROUTE" -n delete -host "$dns_server" "${{dns_gateway:-}}" >/dev/null 2>&1 || "$ROUTE" -n delete -host "$dns_server" >/dev/null 2>&1 || failed=1
  done < "$state_file"

  if [ "$failed" -eq 0 ]; then
    /bin/rm -f "$state_file"
    return 0
  fi

  return 1
}}

failed=0
remove_recorded_dns_routes "$ROUTE_STATE_FILE" || failed=1
if [ -f "$STATE_FILE" ]; then
  tab="$(printf '\t')"
  while IFS="$tab" read -r service current_dns; do
    [ -n "$service" ] || continue
    restore_service_dns "$service" "$current_dns" || failed=1
  done < "$STATE_FILE"
fi

if [ "$failed" -eq 0 ] && [ -f "$STATE_FILE" ]; then
  /bin/rm -f "$STATE_FILE"
elif [ "$failed" -ne 0 ]; then
  printf '%s\n' "OPENWRAP_DNS_WARNING: RESTORE_PENDING_RECONCILE" >&2
fi
flush_dns_cache
[ "$failed" -eq 0 ]
"##,
        state_file = shell_single_quote(state_file),
        route_state_file = shell_single_quote(route_state_file),
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
        let runtime_dir = temp
            .path()
            .join("runtime")
            .join("profile")
            .join("session-split");
        let profile_id = ProfileId::new();
        std::fs::create_dir_all(&runtime_dir).unwrap();
        let mut config = String::from("client\n");

        let cleanup_paths = append_launch_config(
            &mut config,
            &runtime_dir,
            &profile_id,
            &DnsPolicy::SplitDnsPreferred,
        )
        .unwrap();

        assert!(config.contains("script-security 2"));
        assert!(config.contains("route-up "));
        assert!(config.contains("route-pre-down "));
        assert!(config.contains("down "));
        assert!(config.contains("openwrap-dns-up.sh"));
        assert!(config.contains("openwrap-dns-down.sh"));
        assert_eq!(cleanup_paths.len(), 1);
        assert!(cleanup_paths[0].join("openwrap-dns-up.sh").exists());
        assert!(cleanup_paths[0].join("openwrap-dns-down.sh").exists());

        let up_script =
            std::fs::read_to_string(cleanup_paths[0].join("openwrap-dns-up.sh")).unwrap();
        assert!(up_script.contains("collect_match_domains()"));
        assert!(up_script.contains("collect_search_domains()"));
        assert!(up_script.contains("OPENWRAP_DNS_DEBUG: $*\" >&2"));
        assert!(up_script.contains("OPENWRAP_DNS_ERROR: $*\" >&2"));
        assert!(up_script.contains("active network devices:"));
        assert!(up_script.contains("selected active service"));
        assert!(up_script.contains("observed VPN DNS servers:"));
        assert!(up_script.contains("printf 'search %s\\n' \"$domain\""));
        assert!(up_script.contains("apply_global_override()"));
        assert!(up_script.contains("AUTO_PROMOTED_FULL_OVERRIDE"));
        assert!(up_script.contains("if [ \"$current_dns\" = \"$desired_dns\" ]; then"));
        assert!(up_script.contains("\"$active_device_file\""));
        assert!(!up_script.contains("active_devices_file"));
    }

    #[test]
    fn appends_transactional_global_dns_scripts() {
        let temp = tempdir().unwrap();
        let runtime_dir = temp
            .path()
            .join("runtime")
            .join("profile")
            .join("session-full");
        std::fs::create_dir_all(&runtime_dir).unwrap();
        let mut config = String::from("client\n");

        let cleanup_paths = append_launch_config(
            &mut config,
            &runtime_dir,
            &ProfileId::new(),
            &DnsPolicy::FullOverride,
        )
        .unwrap();

        let up_script =
            std::fs::read_to_string(cleanup_paths[0].join("openwrap-dns-up.sh")).unwrap();
        let down_script =
            std::fs::read_to_string(cleanup_paths[0].join("openwrap-dns-down.sh")).unwrap();
        assert!(up_script.contains("OPENWRAP_DNS_DEBUG: $*\" >&2"));
        assert!(up_script.contains("OPENWRAP_DNS_ERROR: $*\" >&2"));
        assert!(up_script.contains("verify_service_dns()"));
        assert!(up_script.contains("rollback_global_state_preserve()"));
        assert!(up_script.contains("active network devices:"));
        assert!(up_script.contains("selected active service"));
        assert!(up_script.contains("observed VPN DNS servers:"));
        assert!(up_script.contains("global DNS override applied successfully"));
        assert!(up_script.contains("if [ \"$current_dns\" = \"$dns_servers\" ]; then"));
        assert!(up_script.contains("\"$active_device_file\""));
        assert!(!up_script.contains("active_devices_file"));
        assert!(down_script.contains("RESTORE_PENDING_RECONCILE"));
    }

    #[test]
    fn skips_script_injection_for_observe_only_profiles() {
        let temp = tempdir().unwrap();
        let runtime_dir = temp
            .path()
            .join("runtime")
            .join("profile")
            .join("session-observe");
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
