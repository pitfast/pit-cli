#!/usr/bin/env python3
import concurrent.futures
import json
import os
import platform
import sys
import threading
import time
import urllib.request
from pathlib import Path


def get_json(url):
    with urllib.request.urlopen(url, timeout=10) as response:
        return json.load(response)


def percentile(values, p):
    if not values:
        return None
    ordered = sorted(values)
    index = min(len(ordered) - 1, round((p / 100) * (len(ordered) - 1)))
    return ordered[index]


def request(base, index, work=None):
    service = ("orders", "users")[index % 2]
    suffix = "/health" if work is None else f"/cpu?work={work}"
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(f"{base}/{service}{suffix}", timeout=30) as response:
            response.read()
            return {"service": service, "status": response.status, "ms": (time.perf_counter() - started) * 1000}
    except Exception as error:
        return {"service": service, "status": 0, "ms": (time.perf_counter() - started) * 1000, "error": str(error)}


def run_scenario(base, control, name, count, concurrency, work=None):
    samples = []
    stop = threading.Event()
    scenario_start_ms = int(time.time() * 1000)

    def sampler():
        while not stop.is_set():
            try:
                snapshot = get_json(f"{control}/v1/cockpit/snapshot")
                samples.append(snapshot)
            except Exception:
                pass
            stop.wait(0.02)

    thread = threading.Thread(target=sampler, daemon=True)
    thread.start()
    started = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as pool:
        results = list(pool.map(lambda index: request(base, index, work), range(count)))
    elapsed = time.perf_counter() - started
    stop.set()
    thread.join(timeout=1)
    try:
        samples.append(get_json(f"{control}/v1/cockpit/snapshot"))
    except Exception:
        pass
    successful = [item for item in results if item["status"] and 200 <= item["status"] < 400]
    # Exclude executions from a prior demo run. The final snapshot matters
    # because very short requests can finish between sampler ticks.
    telemetry_by_execution = {}
    for snapshot in samples:
        for execution in snapshot.get("recent_executions", []):
            if execution.get("completed_at_unix_ms", 0) >= scenario_start_ms:
                telemetry_by_execution[execution["execution_id"]] = execution
    telemetry = list(telemetry_by_execution.values())
    return {
        "name": name,
        "requests": count,
        "concurrency": concurrency,
        "work": work or "health",
        "elapsed_ms": elapsed * 1000,
        "throughput_rps": len(successful) / elapsed if elapsed else 0,
        "failures": len(results) - len(successful),
        "http_ms": {key: percentile([item["ms"] for item in successful], key) for key in (50, 95, 99)},
        "queue_wait_us": {key: percentile([item["queue_wait_us"] for item in telemetry], key) for key in (50, 95, 99)},
        "scheduler_gap_us": {key: percentile([item["scheduler_gap_us"] for item in telemetry], key) for key in (50, 95, 99)},
        "guest_execution_us": {key: percentile([item["guest_execution_us"] for item in telemetry], key) for key in (50, 95, 99)},
        "total_execution_us": {key: percentile([item["total_us"] for item in telemetry], key) for key in (50, 95, 99)},
        "active_lane_peak": max((snapshot["grid"]["running"] for snapshot in samples), default=0),
        "active_stores_peak": max((snapshot["system"]["active_stores"] for snapshot in samples), default=0),
        "cpu_peak_percent": max((snapshot["system"]["process_cpu_percent"] or 0 for snapshot in samples), default=0),
        "rss_peak_bytes": max((snapshot["system"]["process_rss_bytes"] or 0 for snapshot in samples), default=0),
        "per_service": {service: sum(1 for item in successful if item["service"] == service) for service in ("orders", "users")},
    }


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: benchmark.py BASE OUTPUT_DIR")
    base, output = sys.argv[1], Path(sys.argv[2])
    control = os.environ.get("PITFAST_CONTROL_ENDPOINT", "http://127.0.0.1:7081")
    idle = get_json(f"{control}/v1/cockpit/snapshot")
    scenarios = [
        run_scenario(base, control, "single-request", 1, 1),
        run_scenario(base, control, "mixed-light", 24, 4),
        run_scenario(base, control, "burst", 64, 32, "large"),
        run_scenario(base, control, "cpu-mix", 48, 16, "medium"),
    ]
    report = {
        "schema": 1,
        "host": {"os": platform.platform(), "machine": platform.machine(), "cpu_count": os.cpu_count()},
        "pit_lane_endpoint": control,
        "http_endpoint": base,
        "idle": {"queue_depth": idle["grid"]["queue_depth"], "running": idle["grid"]["running"], "services": len(idle["services"])},
        "scenarios": scenarios,
    }
    output.mkdir(parents=True, exist_ok=True)
    (output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    lines = ["# PitFast Cockpit Demo Benchmark", "", f"Host: {report['host']['os']} / {report['host']['machine']} / {report['host']['cpu_count']} CPUs", "", "| Scenario | Req/s | HTTP p50/p95/p99 ms | Queue p95 us | Guest p95 us | Lane peak | CPU peak | RSS peak | Errors |", "|---|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for item in scenarios:
        lines.append(f"| {item['name']} | {item['throughput_rps']:.2f} | {item['http_ms'][50]:.2f}/{item['http_ms'][95]:.2f}/{item['http_ms'][99]:.2f} | {item['queue_wait_us'][95]} | {item['guest_execution_us'][95]} | {item['active_lane_peak']} | {item['cpu_peak_percent']:.1f}% | {item['rss_peak_bytes'] / 1024 / 1024:.1f} MiB | {item['failures']} |")
    lines += ["", "Timing fields are collected from the read-only PitLane Cockpit snapshot. CPU/RSS are process samples from /proc.", "The benchmark does not claim OS page-cache coldness or zero infrastructure memory."]
    (output / "summary.md").write_text("\n".join(lines) + "\n")
    print(output / "summary.md")


if __name__ == "__main__":
    main()
