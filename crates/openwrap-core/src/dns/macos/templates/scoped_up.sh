#!/bin/sh
set -eu

SCOPED_STATE_FILE={{scoped_state_file}}
GLOBAL_STATE_FILE={{global_state_file}}
ROUTE_STATE_FILE={{route_state_file}}
PROFILE_ID={{profile_id}}
# Space-separated domains the user listed for this profile. OpenWrap validates
# each one as a plain DNS hostname before rendering it here.
PROFILE_MATCH_DOMAINS={{profile_match_domains}}
RESOLVER_DIR=/etc/resolver
MARKER="# OpenWrap managed DNS"
PROFILE_MARKER="# profile_id=$PROFILE_ID"

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

pushed_match_domains="$(collect_match_domains)"
search_domains="$(collect_search_domains)"
log_debug "observed VPN match domains: $pushed_match_domains"
log_debug "observed VPN search domains: $search_domains"
log_debug "profile listed domains: $PROFILE_MATCH_DOMAINS"
match_domains="$(printf '%s\n' $pushed_match_domains $PROFILE_MATCH_DOMAINS | /usr/bin/sort -u)"
if [ -z "$(printf '%s%s' "$match_domains" "$search_domains" | /usr/bin/tr -d '[:space:]')" ]; then
  /bin/rm -f "$SCOPED_STATE_FILE"
  log_debug "no VPN domains listed or pushed; leaving system DNS unchanged"
  printf '%s\n' "OPENWRAP_DNS_WARNING: NO_SCOPED_DOMAINS" >&2
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
