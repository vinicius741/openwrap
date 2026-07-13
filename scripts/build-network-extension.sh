#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SOURCE_DIR="$ROOT_DIR/src-tauri/native/macos/packet-tunnel"
BUILD_DIR="${OPENWRAP_PACKET_TUNNEL_BUILD_DIR:-$ROOT_DIR/target/packet-tunnel}"
BUNDLE="$BUILD_DIR/OpenWrapPacketTunnel.systemextension"
VERSION="${OPENWRAP_VERSION:-0.1.0}"
BUILD_NUMBER="${OPENWRAP_BUILD_NUMBER:-1}"

if [ -n "${APPLE_SIGNING_IDENTITY:-}" ] &&
   [ -z "${OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE:-}" ]; then
  echo "APPLE_SIGNING_IDENTITY is set, but OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE is missing." >&2
  echo "A signed Packet Tunnel system extension needs its Developer ID provisioning profile." >&2
  exit 1
fi

if [ -n "${OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE:-}" ] &&
   [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE is set, but APPLE_SIGNING_IDENTITY is missing." >&2
  exit 1
fi

if [ -n "${OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE:-}" ] &&
   [ ! -f "$OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE" ]; then
  echo "Packet Tunnel provisioning profile not found: $OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE" >&2
  exit 1
fi

cmake -S "$SOURCE_DIR" -B "$BUILD_DIR" \
  -DCMAKE_BUILD_TYPE=Release \
  -DOPENWRAP_VERSION="$VERSION" \
  -DOPENWRAP_BUILD_NUMBER="$BUILD_NUMBER"
cmake --build "$BUILD_DIR" --config Release --parallel

if [ -n "${OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE:-}" ]; then
  cp "$OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE" \
    "$BUNDLE/Contents/embedded.provisionprofile"
fi

if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  codesign --force --timestamp --options runtime \
    --entitlements "$SOURCE_DIR/OpenWrapPacketTunnel.entitlements" \
    --sign "$APPLE_SIGNING_IDENTITY" \
    "$BUNDLE"
  codesign --verify --strict --verbose=2 "$BUNDLE"
else
  echo "Built unsigned Packet Tunnel system extension. Set APPLE_SIGNING_IDENTITY to activate it."
fi
