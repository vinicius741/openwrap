#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP_BUNDLE="${1:-$ROOT_DIR/target/release/bundle/macos/OpenWrap.app}"
EXTENSION_BUNDLE="$APP_BUNDLE/Contents/Library/SystemExtensions/app.openwrap.desktop.PacketTunnel.systemextension"

fail() {
  echo "Signing verification failed: $*" >&2
  exit 1
}

[ -d "$APP_BUNDLE" ] || fail "app bundle not found at $APP_BUNDLE"
[ -d "$EXTENSION_BUNDLE" ] || fail "Packet Tunnel extension is not embedded in the app bundle"
[ -f "$APP_BUNDLE/Contents/embedded.provisionprofile" ] ||
  fail "host app provisioning profile is not embedded"
[ -f "$EXTENSION_BUNDLE/Contents/embedded.provisionprofile" ] ||
  fail "Packet Tunnel provisioning profile is not embedded"

codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"

TEMP_DIR=$(mktemp -d)
trap 'rm -rf "$TEMP_DIR"' EXIT HUP INT TERM
APP_ENTITLEMENTS="$TEMP_DIR/app-entitlements.plist"
EXTENSION_ENTITLEMENTS="$TEMP_DIR/extension-entitlements.plist"

codesign -d --entitlements :- "$APP_BUNDLE" >"$APP_ENTITLEMENTS" 2>/dev/null
codesign -d --entitlements :- "$EXTENSION_BUNDLE" >"$EXTENSION_ENTITLEMENTS" 2>/dev/null

[ "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.developer.system-extension.install' "$APP_ENTITLEMENTS" 2>/dev/null || true)" = "true" ] ||
  fail "host signature is missing com.apple.developer.system-extension.install"

/usr/libexec/PlistBuddy \
  -c 'Print :com.apple.developer.networking.networkextension' \
  "$EXTENSION_ENTITLEMENTS" 2>/dev/null |
  grep -q 'packet-tunnel-provider-systemextension' ||
  fail "Packet Tunnel signature is missing packet-tunnel-provider-systemextension"

APP_TEAM=$(codesign -dv --verbose=4 "$APP_BUNDLE" 2>&1 | awk -F= '/^TeamIdentifier=/{print $2}')
EXTENSION_TEAM=$(codesign -dv --verbose=4 "$EXTENSION_BUNDLE" 2>&1 | awk -F= '/^TeamIdentifier=/{print $2}')

[ -n "$APP_TEAM" ] && [ "$APP_TEAM" != "not set" ] || fail "host app is only ad-hoc signed"
[ "$APP_TEAM" = "$EXTENSION_TEAM" ] ||
  fail "host app Team ID ($APP_TEAM) does not match Packet Tunnel Team ID ($EXTENSION_TEAM)"

echo "OpenWrap signing is valid for Packet Tunnel activation (Team ID: $APP_TEAM)."
