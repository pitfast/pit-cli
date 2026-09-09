#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_demo-common.sh"

wait_http "$demo_control/v1/runtime" 5 || fail "PitLane is not running; run scripts/demo-start.sh"
if [ -x "$pit_bin" ] && [ -s "$demo_release_file" ]; then
  "$pit_bin" route stable pitstop-demo /orders --service orders --host "$demo_host" --release "$(head -n 1 "$demo_release_file")" --control-endpoint "$demo_control" >/dev/null
elif [ -x "$pit_bin" ]; then
  "$pit_bin" route clear pitstop-demo /orders --service orders --host "$demo_host" --control-endpoint "$demo_control" >/dev/null
fi
wait_idle 60 || fail "demo did not return to idle; inspect $demo_logs/pit-lane.log"
say "Demo reset: stable/default routing, queue=0, running=0, active Stores=0"
