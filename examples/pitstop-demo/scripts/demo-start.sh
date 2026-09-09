#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_demo-common.sh"

require_file "$pit_bin"
require_file "$lane_bin"
mkdir -p "$demo_state" "$demo_logs" "$PIT_DEPLOYMENT_STATE_DIR" "$PIT_ARTIFACT_STORE_ROOT"

if pid="$(owned_lane_pid 2>/dev/null)"; then
  wait_http "$demo_control/v1/runtime" 5 || fail "owned PitLane PID $pid is not responding"
  say "Reusing PitLane PID $pid"
else
  if curl -fsS --max-time 1 "$demo_control/v1/runtime" >/dev/null 2>&1; then
    fail "ports 7080/7081 are already occupied by a process not owned by this demo"
  fi
  rm -f "$demo_pid_file"
  printf 'pitfast-demo\n' > "$demo_lock_file"
  # A new session keeps the demo-owned infrastructure alive after this
  # helper exits from an ordinary terminal or CI command runner.
  setsid "$lane_bin" \
    --listen 127.0.0.1:7080 \
    --control-listen 127.0.0.1:7081 \
    --state-dir "$PIT_DEPLOYMENT_STATE_DIR" \
    --artifact-store "$PIT_ARTIFACT_STORE_ROOT" \
    >"$demo_logs/pit-lane.log" 2>&1 </dev/null &
  lane_pid=$!
  printf '%s\n' "$lane_pid" > "$demo_pid_file"
  if ! wait_http "$demo_control/v1/runtime" 30; then
    kill "$lane_pid" 2>/dev/null || true
    fail "PitLane did not become healthy; inspect $demo_logs/pit-lane.log"
  fi
  say "Started PitLane PID $lane_pid"
fi

(cd "$demo_root" && "$pit_bin" up -f "$demo_root/app.pit" --timings >"$demo_logs/pit-up.log" 2>&1)
(cd "$demo_root" && "$pit_bin" web --no-open >"$demo_logs/pit-web.log" 2>&1)
"$pit_bin" releases pitstop-demo --control-endpoint "$demo_control" | awk 'NR == 2 {print $1}' >"$demo_release_file"
wait_http "$demo_http/__pit/" 30 || fail "Pit Web did not become reachable"
wait_http "$demo_control/v1/management/snapshot" 10 || fail "management API did not become reachable"
say "PitFast demo ready"
say "Pit Web: $demo_http/__pit/"
say "PitLane: $demo_http"
say "Logs: $demo_logs"
