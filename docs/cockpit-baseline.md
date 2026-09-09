# Cockpit baseline

PitFast already had the operational ingredients needed for a demo: the
PitScheduler exposed queue/running/peak counters, PitLane emitted execution
lifecycle events, and deployment state exposed prepared artifacts. It did not
have a single read-only snapshot suitable for an operator view.

The minimum addition is `GET /v1/cockpit/snapshot` on PitLane's loopback
control listener. The endpoint combines the live scheduler snapshot with
deployment readiness, bounded execution history, real queue/scheduler/guest
timings, and periodic `/proc` CPU/RSS samples. It is observational only; it
cannot deploy, undeploy, rollback, or mutate application state.

The scheduler assignment hook reports the concrete execution and lane after a
lane is reserved. This fixes the previous observability gap where a live lane
snapshot could only expose a placeholder execution id. Scheduling and Lane
admission semantics remain unchanged.

## Snapshot contract

The schema is versioned (`schema: 1`) and contains `grid`, `services`,
`recent_executions`, and `system` sections. Recent executions are bounded to
64 entries. CPU and RSS are best-effort host process measurements; missing
`/proc` data is represented as `null`, never fabricated.

The demo intentionally does not call this an OS page-cache-cold benchmark.
The benchmark measures real request and runtime behavior on the current host.
