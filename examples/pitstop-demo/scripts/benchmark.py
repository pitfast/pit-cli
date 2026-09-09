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


def maximum(values):
    values = [value for value in values if value is not None]
    return max(values) if values else None


def display(value, suffix=""):
    if value is None:
        return "N/A"
    if isinstance(value, float):
        return f"{value:.2f}{suffix}"
    return f"{value}{suffix}"


def memory_bytes():
    try:
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemTotal:"):
                return int(line.split()[1]) * 1024
    except (OSError, ValueError, IndexError):
        pass
    return None


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
        "dispatch_gap_us": {key: percentile([item["dispatch_gap_us"] for item in telemetry], key) for key in (50, 95, 99)},
        "guest_execution_us": {key: percentile([item["guest_execution_us"] for item in telemetry], key) for key in (50, 95, 99)},
        "total_execution_us": {key: percentile([item["total_us"] for item in telemetry], key) for key in (50, 95, 99)},
        # Peak activity is maintained by the scheduler at assignment time;
        # polling `running` can miss short executions entirely.
        "active_lane_peak": maximum(snapshot["grid"].get("peak_active_lanes") for snapshot in samples),
        "active_stores_peak": maximum(snapshot["system"].get("active_stores") for snapshot in samples),
        "cpu_peak_percent": maximum(snapshot["system"].get("process_cpu_percent") for snapshot in samples),
        "cpu_equivalent_cores_peak": maximum(snapshot["system"].get("cpu_equivalent_cores") for snapshot in samples),
        "rss_peak_bytes": maximum(snapshot["system"].get("process_rss_bytes") for snapshot in samples),
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
        "schema": 2,
        "metric_definitions": {
            "queue_wait": "time from request admission to lane assignment while capacity is unavailable",
            "dispatch_gap": "time from lane assignment to guest invocation start",
            "guest_execution": "guest execution duration",
            "internal_total": "PitFast internal execution lifecycle duration",
            "external_http": "client-observed HTTP request duration",
        },
        "host": {"os": platform.platform(), "machine": platform.machine(), "logical_cpus": os.cpu_count(), "memory_bytes": memory_bytes()},
        "pit_lane_endpoint": control,
        "http_endpoint": base,
        "idle": {"queue_depth": idle["grid"]["queue_depth"], "running": idle["grid"]["running"], "services": len(idle["services"])},
        "scenarios": scenarios,
    }
    output.mkdir(parents=True, exist_ok=True)
    (output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    lines = ["# PitFast Cockpit Demo Benchmark", "", f"Host: {report['host']['os']} / {report['host']['machine']} / {report['host']['logical_cpus']} logical CPUs", "", "| Scenario | Req/s | External HTTP p50/p95/p99 ms | Queue wait p95 us | Dispatch gap p95 us | Guest p95 us | Internal total p95 us | Lane peak | Process CPU | RSS peak | Errors |", "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for item in scenarios:
        http = item["http_ms"]
        lines.append(f"| {item['name']} | {item['throughput_rps']:.2f} | {display(http[50], ' ms')}/{display(http[95], ' ms')}/{display(http[99], ' ms')} | {display(item['queue_wait_us'][95])} | {display(item['dispatch_gap_us'][95])} | {display(item['guest_execution_us'][95])} | {display(item['total_execution_us'][95])} | {display(item['active_lane_peak'])} | {display(item['cpu_peak_percent'], '%')} (~{display(item['cpu_equivalent_cores_peak'])} cores) | {display(None if item['rss_peak_bytes'] is None else round(item['rss_peak_bytes'] / 1024 / 1024, 1), ' MiB')} | {item['failures']} |")
    lines += ["", "Queue wait is capacity waiting; dispatch gap is lane assignment to guest start. External HTTP latency includes the client/network boundary; internal timings come from PitLane telemetry.", "Timing fields are collected from the read-only PitLane Cockpit snapshot. CPU/RSS are process samples from /proc.", "The benchmark does not claim OS page-cache coldness or zero infrastructure memory."]
    (output / "summary.md").write_text("\n".join(lines) + "\n")
    print(output / "summary.md")


if __name__ == "__main__":
    main()
