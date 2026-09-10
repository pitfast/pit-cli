#!/usr/bin/env python3
"""Bounded mixed-load generator for one million total demo requests.

This limits in-flight work deliberately. It models a sudden large arrival
volume without pretending that one laptop can hold one million sockets.
"""

import argparse
import json
import queue
import threading
import time
import urllib.request
from collections import Counter


def request(base, index, write_every, timeout):
    if index % write_every == 0:
        operation = "db-write"
        url = f"{base}/orders"
        request = urllib.request.Request(
            url,
            data=json.dumps(
                {"customer": f"burst-{index}", "amount": 1000 + (index % 100000)}
            ).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
    else:
        slot = index % 10
        if slot < 4:
            operation, url = "db-read", f"{base}/orders"
        elif slot < 6:
            operation, url = "db-health", f"{base}/orders/health"
        elif slot < 8:
            operation, url = "user-health", f"{base}/users/health"
        else:
            operation, url = "user-work", f"{base}/users/cpu?work=small"
        request = urllib.request.Request(url, method="GET")

    started = time.perf_counter()
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            response.read()
            status = response.status
        return operation, status, (time.perf_counter() - started) * 1000, None
    except Exception as error:
        return operation, 0, (time.perf_counter() - started) * 1000, str(error)


def run(base, total, concurrency, write_every, timeout, progress_every):
    work = queue.Queue(maxsize=concurrency * 4)
    results = Counter()
    errors = []
    lock = threading.Lock()
    completed = 0
    started = time.perf_counter()
    stop = object()
    print(
        f"starting: total={total} concurrency={concurrency} "
        f"expected_db_writes={(total + write_every - 1) // write_every}",
        flush=True,
    )

    def worker():
        nonlocal completed
        while True:
            index = work.get()
            try:
                if index is stop:
                    return
                operation, status, duration, error = request(
                    base, index, write_every, timeout
                )
                with lock:
                    results[f"{operation}.requests"] += 1
                    results[f"{operation}.ms"] += duration
                    if 200 <= status < 400:
                        results["success"] += 1
                    else:
                        results["failure"] += 1
                        if len(errors) < 10:
                            errors.append(
                                {"operation": operation, "status": status, "error": error}
                            )
                    completed += 1
                    if progress_every and completed % progress_every == 0:
                        elapsed = time.perf_counter() - started
                        print(
                            f"progress: {completed}/{total} "
                            f"({completed / elapsed:.1f} req/s)",
                            flush=True,
                        )
            finally:
                work.task_done()

    threads = [
        threading.Thread(target=worker, name=f"load-worker-{i}", daemon=True)
        for i in range(concurrency)
    ]
    for thread in threads:
        thread.start()

    for index in range(total):
        work.put(index)
    for _ in threads:
        work.put(stop)
    work.join()
    for thread in threads:
        thread.join()

    elapsed = time.perf_counter() - started
    report = {
        "requests": total,
        "concurrency": concurrency,
        "write_every": write_every,
        "expected_db_writes": (total + write_every - 1) // write_every,
        "elapsed_seconds": elapsed,
        "throughput_rps": total / elapsed if elapsed else 0,
        "success": results["success"],
        "failure": results["failure"],
        "operations": {
            operation: {
                "requests": results[f"{operation}.requests"],
                "average_ms": (
                    results[f"{operation}.ms"] / results[f"{operation}.requests"]
                    if results[f"{operation}.requests"]
                    else None
                ),
            }
            for operation in (
                "db-write",
                "db-read",
                "db-health",
                "user-health",
                "user-work",
            )
        },
        "sample_errors": errors,
    }
    print(json.dumps(report, indent=2))
    return 0 if report["failure"] == 0 else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base", nargs="?", default="http://127.0.0.1:7080")
    parser.add_argument("--requests", type=int, default=1_000_000)
    parser.add_argument("--concurrency", type=int, default=64)
    parser.add_argument(
        "--write-every",
        type=int,
        default=1000,
        help="write one order every N arrivals (default: 1000)",
    )
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--progress-every", type=int, default=10_000)
    args = parser.parse_args()
    if args.requests < 1 or args.concurrency < 1 or args.write_every < 1:
        parser.error("requests, concurrency, and write-every must be positive")
    raise SystemExit(
        run(
            args.base,
            args.requests,
            args.concurrency,
            args.write_every,
            args.timeout,
            args.progress_every,
        )
    )


if __name__ == "__main__":
    main()
