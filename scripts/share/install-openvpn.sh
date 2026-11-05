#!/bin/bash
# Shared by recipient setup and the in-app installer. Never run Homebrew as root.
set -euo pipefail

find_openvpn() {
  local candidate
  for candidate in "${1:-}" /opt/homebrew/sbin/openvpn /usr/local/sbin/openvpn /usr/bin/openvpn; do
    if [[ -n "$candidate" && -f "$candidate" && -x "$candidate" ]]; then
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

install_openvpn() (
  set -euo pipefail
  local openvpn brew installer_dir
  if openvpn=$(find_openvpn "${1:-}"); then
    echo "OpenVPN is ready: $openvpn"
    return 0
  fi

  if [[ "$(id -u)" == 0 ]]; then
    echo "Run setup as your normal macOS user, without sudo. Setup asks for your password when needed." >&2
    return 1
  fi

  if ! brew=$(find_brew); then
    echo "Installing Homebrew from https://github.com/Homebrew/install."
    echo "Enter your Mac administrator password when asked. This may also install Apple's Command Line Tools."
    sudo -v
    installer_dir=$(mktemp -d)
    trap "$(printf 'rm -rf -- %q' "$installer_dir")" EXIT
    curl --fail --show-error --location --proto '=https' --tlsv1.2 \
      --output "$installer_dir/install.sh" \
      https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh
    NONINTERACTIVE=1 /bin/bash "$installer_dir/install.sh"
    brew=$(find_brew) || { echo "Homebrew installation did not complete." >&2; return 1; }
  fi

  echo "Installing OpenVPN with Homebrew. This may take several minutes."
  # Finder-launched apps and new Macs may not have Homebrew on PATH yet.
  export PATH="$(dirname "$brew"):/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
  HOMEBREW_NO_AUTO_UPDATE=1 "$brew" install openvpn
  openvpn=$(find_openvpn) || {
    echo "OpenVPN installation did not produce an executable in a supported location." >&2
    return 1
  }
  echo "OpenVPN is ready: $openvpn"
)

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  install_openvpn "${1:-}"
fi
