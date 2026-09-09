#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_demo-common.sh"

out="$demo_root/artifacts/demo"
run_id="$(date -u +%Y%m%dT%H%M%SZ)"
run_dir="$out/runs/$run_id"
mkdir -p "$run_dir" "$out/fallback" "$demo_logs"

"$demo_root/scripts/demo-preflight.sh" >"$run_dir/preflight.txt"
"$demo_root/scripts/demo-start.sh" >"$run_dir/start.txt"
"$demo_root/scripts/demo-reset.sh" >"$run_dir/reset-before.txt"
curl -fsS "$demo_http/__pit/" >"$run_dir/pit-web.html"
curl -fsS "$demo_http/__pit/assets/pit-web.js" >"$run_dir/pit-web.js"
curl -fsS "$demo_http/__pit/assets/pit-web.css" >"$run_dir/pit-web.css"
curl -fsS "$demo_control/v1/management/snapshot" >"$run_dir/management.json"
curl -fsS "$demo_control/v1/cockpit/snapshot" >"$run_dir/idle-before.json"
cp "$run_dir/idle-before.json" "$out/fallback/cockpit-idle.txt"
python3 - "$run_dir/idle-before.json" <<'PY'
import json
import sys
d = json.load(open(sys.argv[1]))
g = d.get("grid", {})
s = d.get("system", {})
if g.get("queue_depth") != 0 or g.get("running") != 0 or s.get("active_stores") != 0:
    raise SystemExit("idle-before is not idle")
PY

"$demo_root/scripts/demo-live.sh" >"$run_dir/live.txt"
"$demo_root/scripts/demo-reset.sh" >"$run_dir/reset-after-live.txt"
( "$demo_root/scripts/demo-burst.sh" medium >"$run_dir/burst.txt" 2>&1 ) &
burst_pid=$!
while kill -0 "$burst_pid" 2>/dev/null; do
  curl -fsS --max-time 2 "$demo_control/v1/cockpit/snapshot" >>"$run_dir/burst-snapshots.ndjson" 2>/dev/null || true
  printf '\n' >>"$run_dir/burst-snapshots.ndjson"
  sleep 0.05
done
wait "$burst_pid"
curl -fsS "$demo_control/v1/cockpit/snapshot" >>"$run_dir/burst-snapshots.ndjson"
cp "$run_dir/burst-snapshots.ndjson" "$out/fallback/cockpit-burst.txt"
"$demo_root/scripts/demo-reset.sh" >"$run_dir/reset-after-burst.txt"
"$demo_root/scripts/demo-routing-short.sh" >"$run_dir/routing.txt"
cp "$run_dir/routing.txt" "$out/fallback/routing-demo-output.txt"
"$demo_root/scripts/demo-reset.sh" >"$run_dir/reset-final.txt"
curl -fsS "$demo_control/v1/cockpit/snapshot" >"$run_dir/idle-after.json"
curl -fsS "$demo_control/v1/management/snapshot" >"$run_dir/management-after.json"
python3 "$demo_root/scripts/snapshot_summary.py" "$run_dir" "$out/rehearsal-latest.json" "$out/rehearsal-latest.md"
cp "$out/rehearsal-latest.md" "$out/fallback/rehearsal-summary.md"
if [ -f "$demo_root/artifacts/bench/summary.md" ]; then cp "$demo_root/artifacts/bench/summary.md" "$out/fallback/benchmark-summary.md"; else printf '# Benchmark evidence\n\nRun scripts/bench-demo.sh after a successful rehearsal.\n' >"$out/fallback/benchmark-summary.md"; fi
printf '# BukuWarung fallback evidence\n\nGenerated from successful rehearsal %s.\n\nThese files are captured local evidence, not simulated dashboard data.\n' "$run_id" >"$out/fallback/README.md"
cat "$out/rehearsal-latest.md"
