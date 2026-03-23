#!/bin/sh
set -eu

STATE_FILE={{state_file}}
ROUTE_STATE_FILE={{route_state_file}}
NETWORKSETUP=/usr/sbin/networksetup
ROUTE=/sbin/route

flush_dns_cache() {
  /usr/bin/dscacheutil -flushcache >/dev/null 2>&1 || true
  /usr/bin/killall -HUP mDNSResponder >/dev/null 2>&1 || true
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
