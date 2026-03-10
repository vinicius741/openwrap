#!/usr/bin/env bash
set -euo pipefail

cargo test -p openwrap-core
npm run build --workspace ui
