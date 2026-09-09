#!/usr/bin/env bash
set -euo pipefail

profile="${1:-medium}"
base="${PITFAST_HTTP_ENDPOINT:-http://127.0.0.1:7080}"
case "$profile" in
  medium) count=24; concurrency=8; work=medium ;;
  burst|heavy) count=64; concurrency=32; work=large ;;
  *) echo "usage: $0 [medium|burst]" >&2; exit 2 ;;
esac

python3 "$(dirname "$0")/load.py" "$base" "$count" "$concurrency" "$work"
