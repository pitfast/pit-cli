#!/usr/bin/env python3
import concurrent.futures
import sys
import time
import urllib.request


def request(base, index, work):
    services = ("orders", "users")
    service = services[index % len(services)]
    url = f"{base}/{service}/cpu?work={work}"
    started = time.perf_counter()
    with urllib.request.urlopen(url, timeout=30) as response:
        body = response.read()
        status = response.status
    return service, status, len(body), (time.perf_counter() - started) * 1000


def main():
    if len(sys.argv) != 5:
        raise SystemExit("usage: load.py BASE COUNT CONCURRENCY WORK")
    base, count, concurrency, work = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
    started = time.perf_counter()
    failures = 0
    durations = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as pool:
        futures = [pool.submit(request, base, index, work) for index in range(count)]
        for future in concurrent.futures.as_completed(futures):
            try:
                service, status, size, duration = future.result()
                durations.append(duration)
                print(f"{service:8} {status} {size:6} bytes {duration:8.2f} ms")
            except Exception as error:
                failures += 1
                print(f"ERROR {error}", file=sys.stderr)
    elapsed = time.perf_counter() - started
    print(f"burst profile: count={count} concurrency={concurrency} work={work} elapsed={elapsed:.3f}s failures={failures}")
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
