#!/bin/sh
set -eu

STATE_FILE={{state_file}}
ROUTE_STATE_FILE={{route_state_file}}
NETWORKSETUP=/usr/sbin/networksetup
SCUTIL=/usr/sbin/scutil
ROUTE=/sbin/route
DEV="${dev:-${1:-}}"
VPN_GATEWAY="${route_vpn_gateway:-}"

collect_dns_servers() {
  foreign_vars=$(/usr/bin/env | /usr/bin/grep '^foreign_option_[0-9][0-9]*=' | /usr/bin/cut -d= -f1 | /usr/bin/sort -t_ -k3,3n || true)
  dns_servers=""

  for var_name in $foreign_vars; do
    value=$(/usr/bin/printenv "$var_name" 2>/dev/null || true)
    case "$value" in
      "dhcp-option DNS "*)
        dns_value=${value#"dhcp-option DNS "}
        dns_value=${dns_value%% *}
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
}

flush_dns_cache() {
  /usr/bin/dscacheutil -flushcache >/dev/null 2>&1 || true
  /usr/bin/killall -HUP mDNSResponder >/dev/null 2>&1 || true
}

log_debug() {
  printf '%s\n' "OPENWRAP_DNS_DEBUG: $*" >&2
}

log_error() {
  printf '%s\n' "OPENWRAP_DNS_ERROR: $*" >&2
}

read_service_dns() {
  service="$1"
  current_dns=$("$NETWORKSETUP" -getdnsservers "$service" 2>/dev/null || true)
  if printf '%s\n' "$current_dns" | /usr/bin/grep -q "There aren't any DNS Servers set on"; then
    printf '__EMPTY__'
  else
    current_dns=$(printf '%s\n' "$current_dns" | /usr/bin/tr '\n' ' ' | /usr/bin/sed 's/[[:space:]]*$//')
    [ -n "$current_dns" ] || current_dns="__EMPTY__"
    printf '%s' "$current_dns"
  fi
}

verify_service_dns() {
  service="$1"
  expected="$2"
  actual="$(read_service_dns "$service")"
  [ "$actual" = "$expected" ]
}

restore_service_dns() {
  service="$1"
  current_dns="$2"

  if [ "$current_dns" = "__EMPTY__" ]; then
    "$NETWORKSETUP" -setdnsservers "$service" Empty >/dev/null 2>&1 || return 1
  else
    set -- $current_dns
    "$NETWORKSETUP" -setdnsservers "$service" "$@" >/dev/null 2>&1 || return 1
  fi

  verify_service_dns "$service" "$current_dns"
}

list_target_services() {
  service_order_file="${STATE_FILE}.services.$$"
  active_device_file="${STATE_FILE}.devices.$$"
  "$SCUTIL" --nwi 2>/dev/null | /usr/bin/awk '/^[[:space:]]*[[:alnum:]][[:alnum]]*[[:space:]]*:/ { device=$1; gsub(":", "", device); print device }' > "$active_device_file"
  active_devices="$(/usr/bin/tr '\n' ' ' < "$active_device_file" | /usr/bin/sed 's/[[:space:]]*$//')"
  [ -n "$active_devices" ] && log_debug "active network devices: $active_devices"
  "$NETWORKSETUP" -listnetworkserviceorder > "$service_order_file" 2>/dev/null || {
    /bin/rm -f "$service_order_file" "$active_device_file"
    return 1
  }

  current_service=""
  while IFS= read -r line; do
    case "$line" in
      \(*\)\ *)
        current_service=${line#*) }
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
}

route_uses_vpn() {
  destination="$1"
  route_output=$("$ROUTE" -n get "$destination" 2>/dev/null || true)
  if [ -n "$DEV" ] && printf '%s\n' "$route_output" | /usr/bin/grep -Eq "interface: $DEV$"; then
    return 0
  fi
  if [ -n "$VPN_GATEWAY" ] && printf '%s\n' "$route_output" | /usr/bin/grep -Eq "gateway: $VPN_GATEWAY$"; then
    return 0
  fi
  return 1
}

ensure_dns_servers_routable() {
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
}

rollback_global_state_preserve() {
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
}

dns_servers="$(collect_dns_servers)"
if [ -z "$dns_servers" ]; then
  /bin/rm -f "$STATE_FILE" "$ROUTE_STATE_FILE"
  exit 0
fi
log_debug "observed VPN DNS servers: $dns_servers"

ensure_dns_servers_routable "$dns_servers" || exit 1
/bin/mkdir -p "$(/usr/bin/dirname "$STATE_FILE")"
tmp_file="${STATE_FILE}.tmp"
services_file="${STATE_FILE}.targets.$$"
cleanup_tmp_files() {
  /bin/rm -f "$services_file"
}
trap cleanup_tmp_files EXIT INT TERM
: > "$tmp_file"

list_target_services > "$services_file" || {
  log_error "failed to enumerate active network services"
  /bin/rm -f "$tmp_file"
  exit 1
}
[ -s "$services_file" ] || {
  log_error "no active network services available for DNS override"
  /bin/rm -f "$tmp_file"
  exit 1
}

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
  "$NETWORKSETUP" -setdnsservers "$service" "$@" >/dev/null 2>&1 || {
    log_error "networksetup failed while applying VPN DNS to service '$service'"
    apply_failed=1
    break
  }
  verify_service_dns "$service" "$dns_servers" || {
    actual_dns="$(read_service_dns "$service" || true)"
    log_error "DNS verification failed for service '$service' (expected='$dns_servers' actual='${actual_dns:-unknown}')"
    apply_failed=1
    break
  }
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
