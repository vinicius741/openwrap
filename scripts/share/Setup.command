#!/bin/sh
# Finder opens .command files in Terminal.
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
"$SCRIPT_DIR/setup.sh" "$@"
result=$?
if [ "$result" -ne 0 ]; then
  echo "Setup did not finish. Review the error above and run Setup.command again."
fi
printf '\nPress Return to close this window. '
read -r answer
exit "$result"
