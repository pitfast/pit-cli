#!/usr/bin/env bash
set -euo pipefail

demo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cli_root="$(cd "$demo_root/../.." && pwd)"
lane_root="$(cd "$cli_root/../pit-lane" && pwd)"
pit_bin="${PITFAST_BIN:-$cli_root/target/release/pit}"
lane_bin="${PITFAST_LANE_BIN:-$lane_root/target/release/pit-lane}"
demo_state="${PITFAST_DEMO_STATE:-$demo_root/artifacts/demo/runtime}"
demo_logs="$demo_state/logs"
demo_pid_file="$demo_state/pit-lane.pid"
demo_lock_file="$demo_state/owner"
demo_release_file="$demo_state/base-release"
demo_http="${PITFAST_HTTP_ENDPOINT:-http://127.0.0.1:7080}"
demo_control="${PITFAST_CONTROL_ENDPOINT:-http://127.0.0.1:7081}"
demo_host="${PITFAST_DEMO_HOST:-demo.localhost}"
demo_listen_addr="${PITFAST_DEMO_LISTEN_ADDR:-0.0.0.0:7080}"
demo_control_listen_addr="${PITFAST_DEMO_CONTROL_LISTEN_ADDR:-0.0.0.0:7081}"
demo_public_host="${PITFAST_DEMO_PUBLIC_HOST:-$(hostname -I 2>/dev/null | awk '{print $1}')}"
demo_public_host="${demo_public_host:-127.0.0.1}"
demo_database_url="${PITFAST_DEMO_DATABASE_URL:-postgresql://pitfast@127.0.0.1:5432/pitfast_demo}"

export PIT_DEPLOYMENT_STATE_DIR="${PIT_DEPLOYMENT_STATE_DIR:-$demo_state/deployments}"
export PIT_ARTIFACT_STORE_ROOT="${PIT_ARTIFACT_STORE_ROOT:-$demo_state/artifacts}"
export PITFAST_HTTP_ENDPOINT="$demo_http"
export PITFAST_CONTROL_ENDPOINT="$demo_control"
export MAIN_DATABASE_URL="$demo_database_url"

say() { printf '%s\n' "$*"; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }

require_file() {
  [ -f "$1" ] || fail "missing $1; run scripts/demo-build.sh"
}

wait_http() {
  local url="$1" timeout_s="${2:-30}" started now
  started="$(date +%s)"
  while :; do
    if curl -fsS --max-time 2 "$url" >/dev/null 2>&1; then return 0; fi
    now="$(date +%s)"
    [ $((now - started)) -lt "$timeout_s" ] || return 1
    sleep 0.2
  done
}

owned_lane_pid() {
  [ -s "$demo_pid_file" ] || return 1
  local pid command_line
  pid="$(tr -cd '0-9' < "$demo_pid_file")"
  [ -n "$pid" ] || return 1
  [ -r "/proc/$pid/cmdline" ] || return 1
  command_line="$(tr '\0' ' ' < "/proc/$pid/cmdline")"
  case "$command_line" in
    *"$lane_bin"*"--listen $demo_listen_addr"*"--control-listen $demo_control_listen_addr"*)
      printf '%s\n' "$pid"; return 0 ;;
  esac
  return 1
}

wait_idle() {
  local timeout_s="${1:-30}" started now snapshot
  started="$(date +%s)"
  while :; do
    if snapshot="$(curl -fsS --max-time 2 "$demo_control/v1/cockpit/snapshot" 2>/dev/null)" \
      && python3 -c 'import json,sys; d=json.load(sys.stdin); g=d.get("grid",{}); s=d.get("system",{}); raise SystemExit(0 if g.get("queue_depth",0)==0 and g.get("running",0)==0 and s.get("active_stores",0)==0 else 1)' <<<"$snapshot"; then
      return 0
    fi
    now="$(date +%s)"
    [ $((now - started)) -lt "$timeout_s" ] || return 1
    sleep 0.25
  done
}
