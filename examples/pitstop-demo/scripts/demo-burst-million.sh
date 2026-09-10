#!/usr/bin/env bash
set -euo pipefail

base="${PITFAST_HTTP_ENDPOINT:-http://127.0.0.1:7080}"
echo "PitFast mixed burst: DB read/write plus user traffic"
echo "DB writes follow --write-every; DB reads and user traffic are mixed"
echo "This is bounded concurrency, not 1,000,000 simultaneous sockets."

curl -fsS --max-time 5 "${base}/orders/health" >/dev/null \
  || { echo "FAIL: orders/PostgreSQL health is unavailable" >&2; exit 1; }

python3 "$(dirname "$0")/load-million.py" "$base" "$@"
