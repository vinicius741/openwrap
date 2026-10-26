#!/bin/sh
# Packages a built OpenWrap.app for distribution (used locally by
# build-share-package.sh and in CI by .github/workflows/release.yml).
#
# Usage: scripts/package-release.sh <OpenWrap.app> <out-dir>
#
# Produces in <out-dir>:
#   OpenWrap-Share.zip        first-time install: app + setup.sh + README-FIRST.txt
#   OpenWrap.app.tar.gz       in-app update archive          (only with a signing key)
#   OpenWrap.app.tar.gz.sig   minisign signature of it       (only with a signing key)
#   latest.json               updater manifest               (only with a signing key)
#
# The app is sealed with an ad-hoc signature and NO entitlements. The repo's
# OpenWrap.entitlements carry restricted system-extension/network-extension
# values that make macOS SIGKILL an ad-hoc signed build at launch; the default
# helper backend needs no entitlements at all. The update archive is built
# AFTER that re-signing, which is why we do not use Tauri's own
# createUpdaterArtifacts (it would archive the app before this step).
#
# Updater signing key (free minisign key, unrelated to Apple signing):
#   TAURI_SIGNING_PRIVATE_KEY            key contents (CI secret), or
#   TAURI_SIGNING_PRIVATE_KEY_PATH       key file (default ~/.tauri/openwrap-updater.key)
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD   key password (default empty)
#
# Other environment:
#   RELEASE_NOTES       text shown in the in-app update banner
#   TAURI_CLI           command used for `signer sign` (default: cargo tauri)
#   SKIP_SMOKE_TEST=1   do not launch the app to check it survives startup
set -eu

if [ $# -ne 2 ]; then
  echo "usage: $0 <OpenWrap.app> <out-dir>" >&2
  exit 64
fi

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP=$(CDPATH= cd -- "$1" && pwd)
OUT_DIR=$2
REPO_URL="https://github.com/vinicius741/openwrap"
APP_BINARY="$APP/Contents/MacOS/openwrap-app"
HELPER_RESOURCE="$APP/Contents/Resources/openwrap-helper-bundled"
TAURI_CLI=${TAURI_CLI:-cargo tauri}

VERSION=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$APP/Contents/Info.plist")

if [ ! -x "$HELPER_RESOURCE" ]; then
  echo "Bundled helper missing: $HELPER_RESOURCE" >&2
  exit 1
fi

echo "==> Packaging OpenWrap $VERSION"

echo "==> Sealing app with ad-hoc signature (no entitlements)"
/usr/bin/codesign --force --sign - "$APP"
/usr/bin/codesign --verify --deep "$APP"

if [ "${SKIP_SMOKE_TEST:-0}" != "1" ]; then
  echo "==> Smoke-testing app launch (a window may flash for ~5 seconds)"
  "$APP_BINARY" >/dev/null 2>&1 &
  SMOKE_PID=$!
  sleep 5
  if ! kill -0 "$SMOKE_PID" 2>/dev/null; then
    echo "Smoke test failed: the app exited or was killed within 5 seconds." >&2
    wait "$SMOKE_PID" 2>/dev/null || true
    exit 1
  fi
  kill "$SMOKE_PID" 2>/dev/null || true
  wait "$SMOKE_PID" 2>/dev/null || true
fi

mkdir -p "$OUT_DIR"
OUT_DIR=$(CDPATH= cd -- "$OUT_DIR" && pwd)
STAGING=$(mktemp -d)
trap 'rm -rf "$STAGING"' EXIT

echo "==> Creating OpenWrap-Share.zip"
SHARE_DIR="$STAGING/OpenWrap-Share"
mkdir -p "$SHARE_DIR"
/usr/bin/ditto "$APP" "$SHARE_DIR/OpenWrap.app"
cp "$ROOT_DIR/scripts/share/setup.sh" "$SHARE_DIR/setup.sh"
chmod +x "$SHARE_DIR/setup.sh"
cp "$ROOT_DIR/scripts/share/README-FIRST.txt" "$SHARE_DIR/README-FIRST.txt"
rm -f "$OUT_DIR/OpenWrap-Share.zip"
/usr/bin/ditto -c -k --keepParent "$SHARE_DIR" "$OUT_DIR/OpenWrap-Share.zip"

DEFAULT_KEY_PATH="$HOME/.tauri/openwrap-updater.key"
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -z "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ] && [ -f "$DEFAULT_KEY_PATH" ]; then
  TAURI_SIGNING_PRIVATE_KEY_PATH=$DEFAULT_KEY_PATH
  export TAURI_SIGNING_PRIVATE_KEY_PATH
fi
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -z "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ]; then
  echo "==> No updater signing key found; skipping update archive and latest.json"
  exit 0
fi
TAURI_SIGNING_PRIVATE_KEY_PASSWORD=${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD

echo "==> Creating update archive OpenWrap.app.tar.gz"
ARCHIVE="$OUT_DIR/OpenWrap.app.tar.gz"
rm -f "$ARCHIVE" "$ARCHIVE.sig"
# COPYFILE_DISABLE keeps macOS tar from adding ._ AppleDouble entries.
COPYFILE_DISABLE=1 /usr/bin/tar -czf "$ARCHIVE" -C "$(dirname "$APP")" "$(basename "$APP")"

echo "==> Signing update archive"
$TAURI_CLI signer sign "$ARCHIVE" >/dev/null
if [ ! -s "$ARCHIVE.sig" ]; then
  echo "Signing failed: $ARCHIVE.sig was not created" >&2
  exit 1
fi

echo "==> Writing latest.json"
SIGNATURE=$(cat "$ARCHIVE.sig")
PUB_DATE=$(date -u +%Y-%m-%dT%H:%M:%SZ)
NOTES=${RELEASE_NOTES:-"OpenWrap $VERSION"}
VERSION="$VERSION" SIGNATURE="$SIGNATURE" PUB_DATE="$PUB_DATE" NOTES="$NOTES" \
  URL="$REPO_URL/releases/download/v$VERSION/OpenWrap.app.tar.gz" \
  node -e '
    const e = process.env
    const manifest = {
      version: e.VERSION,
      notes: e.NOTES,
      pub_date: e.PUB_DATE,
      platforms: { "darwin-aarch64": { signature: e.SIGNATURE, url: e.URL } },
    }
    process.stdout.write(JSON.stringify(manifest, null, 2) + "\n")
  ' > "$OUT_DIR/latest.json"

echo
echo "Release artifacts in $OUT_DIR:"
ls -lh "$OUT_DIR"
