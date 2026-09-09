#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
pit="${PITFAST_BIN:-$root/../../target/release/pit}"
control="${PITFAST_CONTROL_ENDPOINT:-http://127.0.0.1:7081}"
base="${PITFAST_HTTP_ENDPOINT:-http://127.0.0.1:7080}"
host="${PITFAST_DEMO_HOST:-demo.localhost}"
out="${PITFAST_ROUTING_BENCH_OUTPUT:-$root/artifacts/routing-bench}"
blue="${PITFAST_BLUE_RELEASE:-}"
green="${PITFAST_GREEN_RELEASE:-}"

if [[ -z "$blue" || -z "$green" ]]; then
  mapfile -t releases < <("$pit" releases pitstop-demo --control-endpoint "$control" | awk 'NR > 1 {print $1}')
  if (( ${#releases[@]} < 2 )); then
    echo "need PITFAST_BLUE_RELEASE and PITFAST_GREEN_RELEASE, or two deployed pitstop-demo releases" >&2
    exit 2
  fi
  blue="${releases[0]}"
  green="${releases[1]}"
fi

mkdir -p "$out"
export ROUTING_BENCH_BASE="$base"
export ROUTING_BENCH_CONTROL="$control"
export ROUTING_BENCH_HOST="$host"
export ROUTING_BENCH_BLUE="$blue"
export ROUTING_BENCH_GREEN="$green"
export ROUTING_BENCH_OUT="$out"
export ROUTING_BENCH_PIT="$pit"

python3 - <<'PY'
import json
import os
import platform
import statistics
import subprocess
import time
import urllib.request

base = os.environ["ROUTING_BENCH_BASE"]
control = os.environ["ROUTING_BENCH_CONTROL"]
host = os.environ["ROUTING_BENCH_HOST"]
blue = os.environ["ROUTING_BENCH_BLUE"]
green = os.environ["ROUTING_BENCH_GREEN"]
out = os.environ["ROUTING_BENCH_OUT"]
pit = os.environ["ROUTING_BENCH_PIT"]

def route(args):
    subprocess.run([pit, "route", *args, "pitstop-demo", "/orders",
                    "--service", "orders", "--host", host, "--control-endpoint", control],
                   check=True, stdout=subprocess.DEVNULL)

def request(headers):
    request = urllib.request.Request(base + "/orders/health", headers={"Host": host, **headers})
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            body = json.loads(response.read())
        return (time.perf_counter() - started) * 1000, body.get("release"), None
    except Exception as exc:
        return (time.perf_counter() - started) * 1000, None, str(exc)

def percentile(values, p):
    if not values:
        return None
    values = sorted(values)
    index = min(len(values) - 1, round((p / 100) * (len(values) - 1)))
    return values[index]

def snapshot():
    try:
        with urllib.request.urlopen(control + "/v1/cockpit/snapshot", timeout=5) as response:
            return json.loads(response.read())
    except Exception:
        return {}

def scenario(name, args, count, headers):
    route(args)
    before = snapshot()
    before_ids = {item.get("request_id") for item in before.get("recent_executions", [])}
    samples, selected, errors = [], {"blue": 0, "green": 0, "other": 0}, []
    started = time.perf_counter()
    for i in range(count):
        extra = headers(i)
        elapsed, release, error = request(extra)
        samples.append(elapsed)
        if release == "blue": selected["blue"] += 1
        elif release == "green": selected["green"] += 1
        else: selected["other"] += 1
        if error: errors.append(error)
    duration = time.perf_counter() - started
    after = snapshot()
    executions = [item for item in after.get("recent_executions", []) if item.get("request_id") not in before_ids]
    def internal_percentile(field, p):
        return percentile([item[field] / 1000 for item in executions if isinstance(item.get(field), (int, float))], p)
    grid = after.get("grid", {})
    system = after.get("system", {})
    return {
        "name": name, "requests": count, "concurrency": 1,
        "duration_seconds": duration, "throughput_rps": count / duration if duration else None,
        "errors": len(errors), "distribution": selected,
        "external_http_ms": {"p50": percentile(samples, 50), "p95": percentile(samples, 95), "p99": percentile(samples, 99)},
        "internal_metrics": {
            "queue_wait_ms": {"p50": internal_percentile("queue_wait_us", 50), "p95": internal_percentile("queue_wait_us", 95), "p99": internal_percentile("queue_wait_us", 99)},
            "dispatch_gap_ms": {"p50": internal_percentile("dispatch_gap_us", 50), "p95": internal_percentile("dispatch_gap_us", 95), "p99": internal_percentile("dispatch_gap_us", 99)},
            "guest_ms": {"p50": internal_percentile("guest_execution_us", 50), "p95": internal_percentile("guest_execution_us", 95), "p99": internal_percentile("guest_execution_us", 99)},
            "internal_total_ms": {"p50": internal_percentile("total_us", 50), "p95": internal_percentile("total_us", 95), "p99": internal_percentile("total_us", 99)},
            "peak_active_lanes": grid.get("peak_active_lanes"),
            "internal_sample_count": len(executions),
            "cpu_percent": system.get("process_cpu_percent"),
            "cpu_equivalent_cores": system.get("cpu_equivalent_cores"),
            "rss_bytes": system.get("process_rss_bytes"),
        },
    }

results = []
results.append(scenario("stable", ["stable", "--release", blue], 20, lambda _: {}))
results.append(scenario("canary-90-10", ["canary", "--stable", blue, "--candidate", green, "--stable-weight", "90", "--candidate-weight", "10"], 100, lambda i: {"X-Pit-User": f"bench-{i}"}))
results.append(scenario("ab-50-50", ["ab", "--a", blue, "--b", green, "--a-weight", "50", "--b-weight", "50", "--key-header", "X-Pit-User", "--experiment-id", "bench"], 100, lambda i: {"X-Pit-User": f"bench-{i}"}))
results.append(scenario("header-candidate", ["header", "--stable", blue, "--candidate", green], 20, lambda _: {"X-Pit-Variant": "candidate"}))
results.append(scenario("shadow-primary-stable", ["shadow", "--stable", blue, "--candidate", green], 20, lambda _: {}))
route(["stable", "--release", blue])
report = {
    "schema": 1,
    "host": {"os": platform.platform(), "machine": platform.machine(), "python": platform.python_version()},
    "release_ids": {"blue": blue, "green": green},
    "scenarios": results,
    "notes": ["External HTTP is measured by urllib around each request.", "Internal timing fields are only reported for newly observed entries in the bounded Cockpit recent-execution ring; unavailable values remain null."],
}
with open(os.path.join(out, "summary.json"), "w", encoding="utf-8") as handle:
    json.dump(report, handle, indent=2)
with open(os.path.join(out, "summary.md"), "w", encoding="utf-8") as handle:
    handle.write("# PitLane routing benchmark\n\n")
    handle.write(f"Schema: {report['schema']}\n\n")
    handle.write("External HTTP timings are client-observed; internal fields are not inferred.\n\n")
    handle.write("| Scenario | Requests | Req/s | Errors | HTTP p50/p95/p99 ms | Blue | Green | Peak lanes |\n|---|---:|---:|---:|---:|---:|---:|---:|\n")
    for item in results:
        http = item["external_http_ms"]
        peak = item["internal_metrics"]["peak_active_lanes"]
        internal = item["internal_metrics"]["internal_total_ms"]
        handle.write(f"| {item['name']} | {item['requests']} | {item['throughput_rps']:.2f} | {item['errors']} | {http['p50']:.2f}/{http['p95']:.2f}/{http['p99']:.2f} | {item['distribution']['blue']} | {item['distribution']['green']} | {peak if peak is not None else 'N/A'} |\n")
PY

echo "routing benchmark written to $out/summary.json and $out/summary.md"
