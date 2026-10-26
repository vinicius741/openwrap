#!/bin/sh
# Builds OpenWrap locally and packages it on the Desktop:
#
#   ~/Desktop/OpenWrap-Release/OpenWrap-Share.zip   first-time install for another Apple Silicon Mac
#   ~/Desktop/OpenWrap-Release/OpenWrap.app.tar.gz  (+ .sig, latest.json) in-app update files,
#                                                   when ~/.tauri/openwrap-updater.key exists
#
# Normal releases are built by GitHub Actions instead (scripts/release.sh);
# this script is for local testing. See scripts/package-release.sh.
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP="$ROOT_DIR/target/release/bundle/macos/OpenWrap.app"
OUT_DIR="${HOME}/Desktop/OpenWrap-Release"

echo "==> Building OpenWrap (release) and app bundle"
cd "$ROOT_DIR"
npm run tauri:build

if [ ! -d "$APP" ]; then
  echo "Bundle not found at: $APP" >&2
  exit 1
fi

"$ROOT_DIR/scripts/package-release.sh" "$APP" "$OUT_DIR"

echo
echo "Send OpenWrap-Share.zip to the other person; they unzip it and run"
echo "setup.sh (see README-FIRST.txt)."
