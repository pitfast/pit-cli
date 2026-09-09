#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
pit="${PITFAST_BIN:-$root/../../target/release/pit}"
control="${PITFAST_CONTROL_ENDPOINT:-http://127.0.0.1:7081}"
base="${PITFAST_HTTP_ENDPOINT:-http://127.0.0.1:7080}"
host="${PITFAST_DEMO_HOST:-demo.localhost}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

get_release() {
  "$pit" releases pitstop-demo --control-endpoint "$control" |
    awk 'NR == 2 {print $1}'
}

echo "== build blue release =="
"$pit" up -f "$root/app.pit" >/dev/null
blue="$(get_release)"
test -n "$blue"
echo "blue/stable: $blue"

cp -a "$root" "$tmp/green"
rm -rf "$tmp/green"/.pit "$tmp/green"/orders/.pit "$tmp/green"/users/.pit "$tmp/green"/web/.pit
sed -i 's/"release":"blue"/"release":"green"/g' "$tmp/green/orders/app/app.go" "$tmp/green/users/main.py"
"$pit" up -f "$tmp/green/app.pit" >/dev/null
green="$(get_release)"
test -n "$green" && test "$green" != "$blue"
echo "green/candidate: $green"

route() {
  "$pit" route "$@" pitstop-demo /orders --service orders --host "$host" --control-endpoint "$control" >/dev/null
}
check() {
  curl -fsS -H "Host: $host" "$base/orders/health"
  echo
}

echo "== stable =="
route stable --release "$blue"
check

echo "== blue/green candidate =="
route blue-green --stable "$blue" --candidate "$green" --active candidate
check

echo "== canary 90/10 =="
route canary --stable "$blue" --candidate "$green" --stable-weight 90 --candidate-weight 10
for i in $(seq 1 20); do curl -fsS -H "Host: $host" -H "X-Pit-User: user-$i" "$base/orders/health" >/dev/null; done
echo "20 deterministic canary-key requests completed"

echo "== deterministic A/B =="
route ab --a "$blue" --b "$green" --a-weight 50 --b-weight 50 --key-header X-Pit-User --experiment-id checkout-v2
for i in $(seq 1 10); do curl -fsS -H "Host: $host" -H "X-Pit-User: user-$i" "$base/orders/health" >/dev/null; done
echo "10 stable A/B assignments completed"

echo "== header override =="
route header --stable "$blue" --candidate "$green" --header X-Pit-Variant
curl -fsS -H "Host: $host" -H "X-Pit-Variant: candidate" "$base/orders/health"
echo

echo "== safe GET shadow =="
route shadow --stable "$blue" --candidate "$green"
check

echo "== return stable =="
route stable --release "$blue"
check
echo "routing demo complete"
