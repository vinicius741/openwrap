#!/bin/sh
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

collect_match_domains() {
  foreign_vars=$(/usr/bin/env | /usr/bin/grep '^foreign_option_[0-9][0-9]*=' | /usr/bin/cut -d= -f1 | /usr/bin/sort -t_ -k3,3n || true)

  for var_name in $foreign_vars; do
    value=$(/usr/bin/printenv "$var_name" 2>/dev/null || true)
    case "$value" in
      "dhcp-option DOMAIN "*)
        domain_value=${value#"dhcp-option DOMAIN "}
        [ -n "$domain_value" ] && printf '%s\n' "$domain_value"
        ;;
    esac
  done | /usr/bin/tr '[:upper:]' '[:lower:]'
}

collect_search_domains() {
  foreign_vars=$(/usr/bin/env | /usr/bin/grep '^foreign_option_[0-9][0-9]*=' | /usr/bin/cut -d= -f1 | /usr/bin/sort -t_ -k3,3n || true)

  for var_name in $foreign_vars; do
    value=$(/usr/bin/printenv "$var_name" 2>/dev/null || true)
    case "$value" in
      "dhcp-option DOMAIN-SEARCH "*)
        search_values=${value#"dhcp-option DOMAIN-SEARCH "}
        for domain_value in $search_values; do
          [ -n "$domain_value" ] && printf '%s\n' "$domain_value"
        done
        ;;
    esac
  done | /usr/bin/tr '[:upper:]' '[:lower:]'
}

normalize_domain() {
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
  service_order_file="${GLOBAL_STATE_FILE}.services.$$"
  active_device_file="${GLOBAL_STATE_FILE}.devices.$$"
  "$SCUTIL" --nwi 2>/dev/null | /usr/bin/awk '/^[[:space:]]*[[:alnum:]][[:alnum:]]*[[:space:]]*:/ { device=$1; gsub(":", "", device); print device }' > "$active_device_file"
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

apply_global_override() {
  state_file="$1"
  route_state_file="$2"
  desired_dns="$3"

  ensure_dns_servers_routable "$desired_dns" || return 1
  /bin/mkdir -p "$(/usr/bin/dirname "$state_file")"
  tmp_file="${state_file}.tmp"
  services_file="${state_file}.targets.$$"
  cleanup_tmp_files() {
    /bin/rm -f "$services_file"
  }
  trap cleanup_tmp_files EXIT INT TERM
  : > "$tmp_file"

  list_target_services > "$services_file" || {
    log_error "failed to enumerate active network services"
    /bin/rm -f "$tmp_file"
    return 1
  }
  [ -s "$services_file" ] || {
    log_error "no active network services available for DNS override"
    /bin/rm -f "$tmp_file"
    return 1
  }

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
    "$NETWORKSETUP" -setdnsservers "$service" "$@" >/dev/null 2>&1 || {
      log_error "networksetup failed while applying VPN DNS to service '$service'"
      apply_failed=1
      break
    }
    verify_service_dns "$service" "$desired_dns" || {
      actual_dns="$(read_service_dns "$service" || true)"
      log_error "DNS verification failed for service '$service' (expected='$desired_dns' actual='${actual_dns:-unknown}')"
      apply_failed=1
      break
    }
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
}

write_resolver() {
  domain="$1"
  resolver_mode="$2"
  resolver_path="$RESOLVER_DIR/$domain"

  if [ -f "$resolver_path" ]; then
    if ! /usr/bin/grep -q "^$MARKER$" "$resolver_path" || ! /usr/bin/grep -q "^$PROFILE_MARKER$" "$resolver_path"; then
      printf '%s\n' "OPENWRAP_DNS_WARNING: Skipped VPN DNS for domain '$domain' because /etc/resolver/$domain already exists and is not managed by this OpenWrap profile." >&2
      return
    fi
  fi

  tmp_resolver="${resolver_path}.openwrap.$$"
  {
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
  } > "$tmp_resolver"
  /bin/mv "$tmp_resolver" "$resolver_path"
  printf '%s\t%s\n' "$domain" "$resolver_path" >> "$tmp_file"
  log_debug "wrote scoped resolver '$resolver_path' for domain '$domain' using mode '$resolver_mode'"
}

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
tmp_file="${SCOPED_STATE_FILE}.tmp"
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
