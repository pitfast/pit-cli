# v0.13 cold activation and distribution evidence

These measurements were taken on Debian GNU/Linux 13.6, x86_64, with 8
logical CPUs and about 15.5 GiB RAM. They are release-mode measurements and
are not OS-page-cache-cold claims.

The true-unique four-service fixture contained Go `net/http`, JavaScript
Fetch, Python ASGI, and static-web components. The ten-service fixture used
ten distinct static-web artifact digests. Preparation concurrency was bounded
at 2 unless noted.

| Scenario | Before | v0.13 | Notes |
| --- | ---: | ---: | --- |
| Four unique artifacts, cold | 25.236 s | 19.157 s | 4 cold compiles; compiled cache enabled in v0.13 |
| Four services, unchanged repeat | not cache-backed | 0.670 s | 4 prepared-cache hits |
| Ten unique artifacts, cold | 81.081 s | 43.365 s | 10 cold compiles |
| Ten artifacts, restarted warm | synchronous restart ~15.3 s historical | route available before prewarm completes; first request 9.5 ms after prewarm | 10 warm restores |

The four-service cold preparation stage fell from about 14.556 s in the
uncached sequential path to 8.592 s with the v0.13 compiled cache and bounded
parallel preparation. Concurrency 1 measured 24.640 s total on the same
fixture, concurrency 2 measured 19.157 s, concurrency 4 measured 23.622 s
before the cache-backed rerun, and concurrency 8 measured 24.355 s. The
conservative default remains 2: it improved cold activation without the
contention observed at 8, and it limits transient memory pressure from large
components.

PitLane restart now loads the persisted immutable release and route snapshot
without rebuilding or synchronously preparing every artifact. Background
prewarm uses the same digest-keyed singleflight path as a first request. A
corrupt derived `.cwasm` was rejected and rebuilt from the canonical artifact;
the active release JSON and ReleaseId remained intact.

During a concurrency-2 candidate preparation under a tight request loop, the
active route had p50 2.932 ms, p95 5.349 ms, and two client timeouts out of
1,094 samples. This is an engineering observation, not a public SLO; future
work should improve resource isolation for heavy concurrent preparation.

The v0.13 alpha bundle is built by `release/build-alpha-bundle.sh`. It is a
Linux x86_64 bundle with SHA-256 verification and is installed by
`scripts/install-alpha.sh` into a versioned user-local directory. No hosted
release or signing key exists yet; local HTTP hosting was used to exercise
the download path.
