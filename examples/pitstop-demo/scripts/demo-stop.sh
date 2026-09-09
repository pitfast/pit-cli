#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_demo-common.sh"

if pid="$(owned_lane_pid 2>/dev/null)"; then
  kill "$pid"
  for _ in $(seq 1 50); do
    if [ ! -r "/proc/$pid/cmdline" ]; then break; fi
    sleep 0.1
  done
  if [ -r "/proc/$pid/cmdline" ]; then kill -KILL "$pid"; fi
  say "Stopped demo-owned PitLane PID $pid"
else
  if [ -e "$demo_pid_file" ]; then rm -f "$demo_pid_file"; fi
  say "No demo-owned PitLane process to stop"
fi
rm -f "$demo_pid_file" "$demo_lock_file"
say "Build caches and benchmark evidence were preserved."
