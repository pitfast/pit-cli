#!/usr/bin/env bash
set -euo pipefail

base="${PITFAST_HTTP_ENDPOINT:-http://127.0.0.1:7080}"
echo "PitFast live demo against ${base}"
curl -fsS "${base}/orders/health"; echo
curl -fsS "${base}/users/health"; echo
curl -fsS "${base}/" >/dev/null

curl -fsS -X POST -H 'Content-Type: application/json' \
  --data '{"customer":"demo-walkthrough","amount":50000}' \
  "${base}/orders"; echo
curl -fsS "${base}/orders"; echo

for service in orders users; do
  curl -fsS "${base}/${service}/cpu?work=medium" >/dev/null &
done
wait
echo "Live demo requests completed. Keep 'pit cockpit' open for the flow."
