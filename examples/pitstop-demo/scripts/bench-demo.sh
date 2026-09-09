#!/usr/bin/env bash
set -euo pipefail

base="${PITFAST_HTTP_ENDPOINT:-http://127.0.0.1:7080}"
out="${PITFAST_BENCH_OUTPUT:-$(dirname "$0")/../artifacts/bench}"
mkdir -p "$out"
python3 "$(dirname "$0")/benchmark.py" "$base" "$out"
