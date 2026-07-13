#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP_BUNDLE="$ROOT_DIR/target/release/bundle/macos/OpenWrap.app"
EXTENSION_SOURCE="${OPENWRAP_PACKET_TUNNEL_BUILD_DIR:-$ROOT_DIR/target/packet-tunnel}/OpenWrapPacketTunnel.systemextension"
EXTENSION_DEST="$APP_BUNDLE/Contents/Library/SystemExtensions/app.openwrap.desktop.PacketTunnel.systemextension"

: "${APPLE_SIGNING_IDENTITY:?Set APPLE_SIGNING_IDENTITY to a Developer ID Application identity}"
: "${OPENWRAP_APP_PROVISIONING_PROFILE:?Set OPENWRAP_APP_PROVISIONING_PROFILE to the host app provisioning profile}"
: "${OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE:?Set OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE to the provider provisioning profile}"

[ -f "$OPENWRAP_APP_PROVISIONING_PROFILE" ] || {
  echo "Host app provisioning profile not found: $OPENWRAP_APP_PROVISIONING_PROFILE" >&2
  exit 1
}

[ -f "$OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE" ] || {
  echo "Packet Tunnel provisioning profile not found: $OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE" >&2
  exit 1
}

cd "$ROOT_DIR"

# Build and sign the Packet Tunnel system extension first so the host bundle can
# embed a complete, already-signed nested binary.
./scripts/build-network-extension.sh

[ -d "$EXTENSION_SOURCE" ] || {
  echo "Packet Tunnel system extension was not produced at $EXTENSION_SOURCE" >&2
  exit 1
}

cargo tauri build --bundles app

[ -d "$APP_BUNDLE" ] || {
  echo "Tauri did not produce an app bundle at $APP_BUNDLE" >&2
  exit 1
}

# Embed the system extension at the Apple-required SystemExtensions path and
# rename it to the provider bundle identifier.
rm -rf "$APP_BUNDLE/Contents/Library/SystemExtensions"
mkdir -p "$APP_BUNDLE/Contents/Library/SystemExtensions"
rm -rf "$EXTENSION_DEST"
cp -R "$EXTENSION_SOURCE" "$EXTENSION_DEST"

cp "$OPENWRAP_APP_PROVISIONING_PROFILE" \
  "$APP_BUNDLE/Contents/embedded.provisionprofile"

# Embedding the extension and host profile changes the bundle after Tauri's
# signing pass, so seal nested content then the containing app again.
codesign --force --timestamp --options runtime \
  --entitlements "$ROOT_DIR/src-tauri/native/macos/packet-tunnel/OpenWrapPacketTunnel.entitlements" \
  --sign "$APPLE_SIGNING_IDENTITY" \
  "$EXTENSION_DEST"

codesign --force --timestamp --options runtime \
  --entitlements "$ROOT_DIR/src-tauri/OpenWrap.entitlements" \
  --sign "$APPLE_SIGNING_IDENTITY" \
  "$APP_BUNDLE"

"$ROOT_DIR/scripts/verify-network-extension-signing.sh" "$APP_BUNDLE"
echo "Signed app ready for connection testing: $APP_BUNDLE"
