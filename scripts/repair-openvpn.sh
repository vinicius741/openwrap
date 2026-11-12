#!/bin/bash
# Run by the in-app "Repair OpenVPN" action. Updates the Homebrew OpenVPN that
# OpenWrap launches and reinstalls it if it still cannot start, which is the
# usual fix after a Homebrew update leaves OpenVPN broken or outdated.
# Never run Homebrew as root.
set -euo pipefail

find_openvpn() {
  local candidate
  for candidate in /opt/homebrew/sbin/openvpn /usr/local/sbin/openvpn; do
    if [[ -f "$candidate" && -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

find_brew() {
  local candidate
  for candidate in /opt/homebrew/bin/brew /usr/local/bin/brew; do
    if [[ -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  command -v brew
}

# Prints the version when the binary starts, e.g. "2.7.9".
openvpn_version() {
  local banner
  banner=$("$1" --version 2>/dev/null | head -n 1) || true
  [[ "$banner" == "OpenVPN "* ]] || return 1
  printf '%s\n' "$banner" | cut -d' ' -f2
}

working_openvpn() {
  local openvpn version
  openvpn=$(find_openvpn) || return 1
  version=$(openvpn_version "$openvpn") || return 1
  printf '%s (%s)\n' "$openvpn" "$version"
}

repair_openvpn() (
  set -euo pipefail
  local brew ready

  if [[ "$(id -u)" == 0 ]]; then
    echo "Run this as your normal macOS user, without sudo." >&2
    return 1
  fi

  if ! brew=$(find_brew); then
    echo "Homebrew is not installed, so OpenVPN cannot be repaired automatically." >&2
    echo "Use Install OpenVPN in OpenWrap's settings instead." >&2
    return 1
  fi
  # Finder-launched apps may not have Homebrew on PATH.
  export PATH="$(dirname "$brew"):/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

  echo "Refreshing Homebrew's package list."
  "$brew" update || echo "Could not refresh the package list; continuing with what is already downloaded."

  if "$brew" list --formula openvpn >/dev/null 2>&1; then
    echo "Updating OpenVPN with Homebrew. Homebrew may also update packages OpenVPN depends on."
    "$brew" upgrade openvpn
  else
    echo "Installing OpenVPN with Homebrew. This may take several minutes."
    "$brew" install openvpn
  fi

  if ! ready=$(working_openvpn); then
    echo "OpenVPN still does not start. Reinstalling it."
    "$brew" reinstall openvpn
    ready=$(working_openvpn) || {
      echo "OpenVPN still does not start after reinstalling. Review the output above." >&2
      return 1
    }
  fi
  echo "OpenVPN is ready: $ready"
)

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  repair_openvpn
fi
