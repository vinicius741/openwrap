#!/bin/sh
# Recipient-side setup for the OpenWrap share package.
#
# Run this from inside the unzipped OpenWrap-Share folder:
#   ./setup.sh
# or pass an explicit app path if you already moved it:
#   ./setup.sh /Applications/OpenWrap.app
#
# It clears the macOS quarantine flag, installs Homebrew and OpenVPN if
# needed, and installs the privileged setuid helper bundled
# inside OpenWrap.app.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
APP="${1:-$SCRIPT_DIR/OpenWrap.app}"
HELPER_RESOURCE="$APP/Contents/Resources/openwrap-helper-bundled"
HELPER_DESTINATION_DIR="/Library/PrivilegedHelperTools"
HELPER_DESTINATION="$HELPER_DESTINATION_DIR/app.openwrap.desktop.openwrap-helper"

echo "OpenWrap setup"
echo "=============="

if [ ! -d "$APP" ]; then
  echo "OpenWrap.app not found at: $APP" >&2
  echo "Run this script from inside the shared folder, or pass the app path:" >&2
  echo "  ./setup.sh /path/to/OpenWrap.app" >&2
  exit 1
fi

if [ ! -x "$HELPER_RESOURCE" ]; then
  echo "Bundled helper missing or not executable:" >&2
  echo "  $HELPER_RESOURCE" >&2
  exit 1
fi

ARCH=$(uname -m)
if [ "$ARCH" != "arm64" ]; then
  echo "This build is Apple Silicon (arm64) only; this Mac is $ARCH." >&2
  exit 1
fi

echo "App: $APP"
echo

# The app is an unsigned hobby build, so strip the quarantine flag macOS
# added during transfer instead of requiring a right-click "Open" dance.
echo "Clearing macOS quarantine flags..."
/usr/bin/xattr -cr "$APP" || true
echo "Done."
echo

# Stop if dependency installation fails so setup never reports false success.
/bin/bash "$SCRIPT_DIR/install-openvpn.sh"
echo

# Same install layout as scripts/install-helper.sh in the repository.
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
  SUDO="sudo"
  echo "Installing the privileged helper (administrator password required)..."
fi

$SUDO /usr/bin/install -d -o root -g wheel -m 755 "$HELPER_DESTINATION_DIR"
$SUDO /usr/bin/install -o root -g wheel -m 4755 "$HELPER_RESOURCE" "$HELPER_DESTINATION"

echo "Helper installed:"
/bin/ls -l "$HELPER_DESTINATION"
echo
echo "Setup complete."
echo "1. Drag OpenWrap.app to /Applications (optional but recommended)."
echo "2. Open OpenWrap, import a .ovpn profile, and connect."
