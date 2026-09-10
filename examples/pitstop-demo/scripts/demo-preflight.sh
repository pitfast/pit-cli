#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_demo-common.sh"

pass=0
warn=0
failures=0
pass_check() { printf 'PASS  %s\n' "$*"; pass=$((pass + 1)); }
warn_check() { printf 'WARN  %s\n' "$*"; warn=$((warn + 1)); }
fail_check() { printf 'FAIL  %s\n' "$*"; failures=$((failures + 1)); }
check_command() {
  if command -v "$1" >/dev/null 2>&1; then pass_check "$1 available: $(command -v "$1")"; else fail_check "$1 is required"; fi
}

say "PitFast BukuWarung demo preflight"
for repo in pit-box pit-crew pit-cli pit-lane pit-paddock pit-circuit; do
  if [ -d "$cli_root/../$repo/.git" ]; then
    if [ -z "$(git -C "$cli_root/../$repo" status --short)" ]; then
      pass_check "$repo worktree clean"
    else
      warn_check "$repo worktree has local changes"
    fi
  else
    fail_check "missing repository $repo"
  fi
done
for command in cargo go python3 node npm curl docker; do check_command "$command"; done
if [ -f "$pit_bin" ]; then pass_check "pit binary: $pit_bin"; else fail_check "missing pit binary: $pit_bin"; fi
if [ -f "$lane_bin" ]; then pass_check "pit-lane binary: $lane_bin"; else fail_check "missing pit-lane binary: $lane_bin"; fi
mkdir -p "$demo_logs" "$PIT_DEPLOYMENT_STATE_DIR" "$PIT_ARTIFACT_STORE_ROOT"
[ -w "$demo_state" ] && pass_check "demo state writable: $demo_state" || fail_check "demo state is not writable: $demo_state"
pass_check "HTTP listener target: $demo_listen_addr"
pass_check "control listener target: $demo_control_listen_addr"
case "$demo_control_listen_addr" in
  127.*|\[::1\]:*|::1:*) ;;
  *) warn_check "public control API enabled; use only on a trusted isolated network" ;;
esac
if command -v docker >/dev/null 2>&1; then
  if docker inspect "${PITFAST_DEMO_POSTGRES_CONTAINER:-pitfast-postgres-demo}" >/dev/null 2>&1 \
    && [ "$(docker inspect -f '{{.State.Running}}' "${PITFAST_DEMO_POSTGRES_CONTAINER:-pitfast-postgres-demo}")" = "true" ]; then
    pass_check "demo PostgreSQL container is running"
  else
    fail_check "demo PostgreSQL container is not running; run scripts/demo-postgres.sh"
  fi
fi
if command -v df >/dev/null 2>&1; then
  free_kib="$(df -Pk "$demo_root" | awk 'NR==2 {print $4}')"
  if [ "${free_kib:-0}" -ge 1048576 ]; then pass_check "disk space >= 1 GiB"; else warn_check "less than 1 GiB free on demo filesystem"; fi
fi
if command -v nproc >/dev/null 2>&1; then pass_check "host logical CPUs: $(nproc)"; else warn_check "nproc unavailable"; fi
if curl -fsS --max-time 2 "$demo_control/v1/runtime" >/dev/null 2>&1; then
  pass_check "PitLane control endpoint reachable: $demo_control"
  if wait_http "$demo_control/v1/management/snapshot" 10; then pass_check "management endpoint reachable"; else fail_check "management endpoint unavailable"; fi
  if wait_http "$demo_control/v1/cockpit/snapshot" 10; then pass_check "Cockpit endpoint reachable"; else fail_check "Cockpit endpoint unavailable"; fi
else
  if owned_lane_pid >/dev/null 2>&1; then fail_check "owned PitLane process is not healthy"; else warn_check "PitLane is not running; run scripts/demo-start.sh"; fi
fi
if (exec 9>"$demo_state/port-check.lock") 2>/dev/null; then exec 9>&-; pass_check "demo state lock is usable"; else fail_check "demo state directory cannot be locked"; fi

printf '\nSummary: PASS=%s WARN=%s FAIL=%s\n' "$pass" "$warn" "$failures"
if [ "$failures" -eq 0 ]; then say "DEMO READY: YES"; else say "DEMO READY: NO"; exit 1; fi
