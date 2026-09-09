#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_demo-common.sh"

# This is a focused integration proof, not part of the three-service hero
# fixture. It uses PitFast's existing native WIT service capability and
# logical pit:// invocation so the proof never depends on service DNS.
require_file "$pit_bin"

tmp="$(mktemp -d "$demo_root/artifacts/demo/nested-release.XXXXXX")"
trap 'if [ -n "${nested_pid:-}" ]; then kill "$nested_pid" 2>/dev/null || true; fi; rm -rf "$tmp"' EXIT
nested_http="http://127.0.0.1:7082"
nested_control="http://127.0.0.1:7083"
nested_state="$tmp/state"
mkdir -p "$nested_state/logs" "$nested_state/deployments" "$nested_state/artifacts"
setsid "$lane_bin" \
  --listen 127.0.0.1:7082 \
  --control-listen 127.0.0.1:7083 \
  --state-dir "$nested_state/deployments" \
  --artifact-store "$nested_state/artifacts" \
  >"$nested_state/logs/pit-lane.log" 2>&1 &
nested_pid=$!
wait_http "$nested_control/v1/runtime" 30 || fail "nested proof PitLane did not become healthy"
mkdir -p "$tmp/blue" "$tmp/green"
copy_component() {
  local source="$1" destination="$2"
  mkdir -p "$destination"
  cp "$source/Cargo.toml" "$destination/"
  [ ! -f "$source/Cargo.lock" ] || cp "$source/Cargo.lock" "$destination/"
  cp -a "$source/src" "$destination/"
}
copy_component "$lane_root/examples/service-a" "$tmp/blue/orders"
copy_component "$lane_root/examples/service-b" "$tmp/blue/users"
mkdir -p "$tmp/green"

python3 - "$tmp/blue/orders/src/lib.rs" "$tmp/blue/users/src/lib.rs" "$lane_root/crates/pit-lane-core/wit" <<'PY'
from pathlib import Path
import sys

orders = Path(sys.argv[1])
users = Path(sys.argv[2])
text = orders.read_text().replace('path: "../../crates/pit-lane-core/wit"', f'path: "{sys.argv[3]}"').replace('pit://service-b', 'pit://users')
text = text.replace('route == "/call-b" &&', '(route == "/call-b" || route == "/orders/call-b") &&')
orders.write_text(text)
text = users.read_text().replace('b"service-b"', 'b"users-blue"')
marker = '(wasip2::http::types::Method::Get, "/hello") => (200, b"users-blue".to_vec()),'
replacement = '(wasip2::http::types::Method::Get, "/hello") => { std::thread::sleep(std::time::Duration::from_millis(250)); (200, b"users-blue".to_vec()) },'
if marker not in text:
    raise SystemExit("users fixture hello marker not found")
users.write_text(text.replace(marker, replacement))
PY
cp -a "$tmp/blue/." "$tmp/green/"
sed -i 's/users-blue/users-green/g' "$tmp/green/users/src/lib.rs"

cat >"$tmp/blue/app.pit" <<EOF
schema = 1
name = "nested-release-proof"

[services.orders]
build = "orders"
language = "rust"
abi = "wasi-preview2"
world = "wasi:http/proxy"

[services.users]
build = "users"
language = "rust"
abi = "wasi-preview2"
world = "wasi:http/proxy"

[routes]
"/orders" = "orders"
"/users" = "users"
EOF
cp "$tmp/blue/app.pit" "$tmp/green/app.pit"

(cd "$tmp/blue" && PIT_DEPLOYMENT_STATE_DIR="$nested_state/deployments" PIT_ARTIFACT_STORE_ROOT="$nested_state/artifacts" "$pit_bin" up -f app.pit --control-endpoint "$nested_control" >"$demo_logs/nested-blue-up.log" 2>&1)
blue="$(PIT_DEPLOYMENT_STATE_DIR="$nested_state/deployments" PIT_ARTIFACT_STORE_ROOT="$nested_state/artifacts" "$pit_bin" releases nested-release-proof --control-endpoint "$nested_control" | awk 'NR == 2 {print $1}')"
test -n "$blue"
(cd "$tmp/green" && PIT_DEPLOYMENT_STATE_DIR="$nested_state/deployments" PIT_ARTIFACT_STORE_ROOT="$nested_state/artifacts" "$pit_bin" up -f app.pit --control-endpoint "$nested_control" >"$demo_logs/nested-green-up.log" 2>&1)
green="$(PIT_DEPLOYMENT_STATE_DIR="$nested_state/deployments" PIT_ARTIFACT_STORE_ROOT="$nested_state/artifacts" "$pit_bin" releases nested-release-proof --control-endpoint "$nested_control" | awk 'NR == 2 {print $1}')"
test -n "$green" && test "$green" != "$blue"

route_stable() {
  "$pit_bin" route stable nested-release-proof /orders --service orders --host nested.localhost --release "$1" --control-endpoint "$nested_control" >/dev/null
}
call_nested() {
  curl -fsS -H 'Host: nested.localhost' "$nested_http/orders/call-b"
}

route_stable "$blue"
blue_body="$(call_nested)"
case "$blue_body" in *users-blue*) ;; *) fail "blue nested call did not resolve blue users: $blue_body" ;; esac
route_stable "$green"
green_body="$(call_nested)"
case "$green_body" in *users-green*) ;; *) fail "green nested call did not resolve green users: $green_body" ;; esac

route_stable "$blue"
slow_file="$tmp/in-flight.txt"
(call_nested >"$slow_file") &
request_pid=$!
sleep 0.05
route_stable "$green"
wait "$request_pid"
inflight="$(cat "$slow_file")"
case "$inflight" in *users-blue*) ;; *) fail "in-flight blue request changed release: $inflight" ;; esac
new_body="$(call_nested)"
case "$new_body" in *users-green*) ;; *) fail "new request did not resolve green users: $new_body" ;; esac

printf 'BLUE orders -> BLUE users: PASS (%s)\nGREEN orders -> GREEN users: PASS (%s)\nIN-FLIGHT BLUE after switch -> BLUE users: PASS (%s)\nNEW after switch -> GREEN users: PASS (%s)\nlogical invocation: pit://users\n' \
  "$blue_body" "$green_body" "$inflight" "$new_body"
