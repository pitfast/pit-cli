#!/usr/bin/env bash
set -euo pipefail

# The full routing script remains the regression matrix. This short variant
# is the meeting-safe proof: stable -> candidate -> stable, through one
# listener, without making every experiment part of the hero narrative.
root="$(cd "$(dirname "$0")/.." && pwd)"
pit="${PITFAST_BIN:-$(cd "$root/../../" && pwd)/target/release/pit}"
control="${PITFAST_CONTROL_ENDPOINT:-http://127.0.0.1:7081}"
base="${PITFAST_HTTP_ENDPOINT:-http://127.0.0.1:7080}"
host="${PITFAST_DEMO_HOST:-demo.localhost}"

mkdir -p "$root/artifacts/demo"
build_log="${PITFAST_ROUTING_LOG:-$root/artifacts/demo/routing-short-build.log}"
"$root/scripts/demo-routing.sh" >"$build_log"
blue="$(awk '/blue\/stable:/ {print $2}' "$build_log")"
green="$(awk '/green\/candidate:/ {print $2}' "$build_log")"
test -n "$blue" && test -n "$green"
"$pit" route blue-green pitstop-demo /orders --service orders --host "$host" --stable "$blue" --candidate "$green" --active candidate --control-endpoint "$control" >/dev/null
candidate="$(curl -fsS -H "Host: $host" "$base/orders/health")"
"$pit" route stable pitstop-demo /orders --service orders --host "$host" --release "$blue" --control-endpoint "$control" >/dev/null
stable="$(curl -fsS -H "Host: $host" "$base/orders/health")"
printf 'stable=%s\ncandidate=%s\nrestored=stable\n' "$stable" "$candidate"
