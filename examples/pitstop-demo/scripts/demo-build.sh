#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_demo-common.sh"

say "Building PitFast demo prerequisites"
command -v cargo >/dev/null 2>&1 || fail "cargo is required"
command -v npm >/dev/null 2>&1 || fail "npm is required to verify the Pit Web bundle"

if [ -f "$cli_root/apps/pit-web/package-lock.json" ]; then
  (cd "$cli_root/apps/pit-web" && npm ci --ignore-scripts)
  (cd "$cli_root/apps/pit-web" && npm run build)
fi
(cd "$lane_root" && cargo build --workspace --release)
(cd "$cli_root" && cargo build --release)

mkdir -p "$demo_logs" "$PIT_DEPLOYMENT_STATE_DIR" "$PIT_ARTIFACT_STORE_ROOT"
printf 'Pit binary: %s\nPitLane binary: %s\nPit Web source bundle: %s\nDemo manifest: %s\n' \
  "$pit_bin" "$lane_bin" "$cli_root/apps/pit-web/dist" "$demo_root/app.pit"
say "Build complete; the meeting flow can run from these local artifacts."
