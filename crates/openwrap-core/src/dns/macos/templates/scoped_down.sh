#!/bin/sh
set -eu

SCOPED_STATE_FILE={scoped_state_file}
GLOBAL_STATE_FILE={global_state_file}
ROUTE_STATE_FILE={route_state_file}
PROFILE_ID={profile_id}
NETWORKSETUP=/usr/sbin/networksetup
ROUTE=/sbin/route
MARKER="# OpenWrap managed DNS"
PROFILE_MARKER="# profile_id=$PROFILE_ID"

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

remove_recorded_dns_routes() {
  state_file="$1"
  [ -f "$state_file" ] || return 0

  failed=0
  tab="$(printf '\t')"
  while IFS="$tab" read -r dns_server dns_gateway; do
    [ -n "$dns_server" ] || continue
    "$ROUTE" -n delete -host "$dns_server" "${dns_gateway:-}" >/dev/null 2>&1 || "$ROUTE" -n delete -host "$dns_server" >/dev/null 2>&1 || failed=1
  done < "$state_file"

  if [ "$failed" -eq 0 ]; then
    /bin/rm -f "$state_file"
    return 0
  fi

  return 1
}

restore_global_state() {
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
}

remove_scoped_resolvers() {
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
}

failed=0
remove_scoped_resolvers "$SCOPED_STATE_FILE" || failed=1
remove_recorded_dns_routes "$ROUTE_STATE_FILE" || failed=1
restore_global_state "$GLOBAL_STATE_FILE" || failed=1
flush_dns_cache
[ "$failed" -eq 0 ]
