#!/usr/bin/env bash
set -euo pipefail

base="${PITFAST_HTTP_ENDPOINT:-http://127.0.0.1:7080}"
curl -fsS "$base/orders/health"; echo
curl -fsS "$base/orders/db"; echo
curl -fsS "$base/orders"; echo
