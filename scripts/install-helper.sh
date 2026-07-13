#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DEFAULT_SOURCE="$ROOT_DIR/target/release/openwrap-helper"
SOURCE="${1:-$DEFAULT_SOURCE}"
DESTINATION_DIR="/Library/PrivilegedHelperTools"
DESTINATION="$DESTINATION_DIR/app.openwrap.desktop.openwrap-helper"

if [ "$(id -u)" -ne 0 ]; then
  echo "This installer needs administrator privileges." >&2
  echo "Run: sudo ./scripts/install-helper.sh" >&2
  exit 1
fi

if [ ! -f "$SOURCE" ] || [ ! -x "$SOURCE" ]; then
  echo "Helper binary not found or not executable: $SOURCE" >&2
  echo "Build it first with: cargo build -p openwrap-helper --release" >&2
  exit 1
fi

# Resolve to a canonical absolute path for confirmation and path checks.
SOURCE=$(cd -- "$(dirname -- "$SOURCE")" && pwd)/$(basename -- "$SOURCE")
SOURCE_BASENAME=$(basename -- "$SOURCE")

if [ "$SOURCE_BASENAME" != "openwrap-helper" ]; then
  echo "Refusing to install '$SOURCE_BASENAME'." >&2
  echo "Expected a binary named openwrap-helper." >&2
  exit 1
fi

case "$SOURCE" in
  "$ROOT_DIR/target/"*)
    ;;
  *)
    echo "Refusing to install a binary outside the repository target directory." >&2
    echo "Resolved source: $SOURCE" >&2
    echo "Allowed prefix:  $ROOT_DIR/target/" >&2
    echo "Build with: cargo build -p openwrap-helper --release" >&2
    exit 1
    ;;
esac

echo "Installing OpenWrap helper from:"
echo "  $SOURCE"
echo "to:"
echo "  $DESTINATION"

/usr/bin/install -d -o root -g wheel -m 755 "$DESTINATION_DIR"
/usr/bin/install -o root -g wheel -m 4755 "$SOURCE" "$DESTINATION"

echo "Installed OpenWrap helper at $DESTINATION"
/bin/ls -l "$DESTINATION"
