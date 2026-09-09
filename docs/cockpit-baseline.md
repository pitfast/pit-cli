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

`grid.peak_active_lanes` is event-derived by PitBox when a lane is assigned;
it is not reconstructed from Cockpit polling. `queue_wait_us` ends at lane
assignment. `dispatch_gap_us` starts at lane assignment and ends when the
guest invocation begins. `guest_execution_us` and `total_us` are internal
PitFast timings; they must not be compared directly with external HTTP
latency, which includes the client and transport boundary.

The demo intentionally does not call this an OS page-cache-cold benchmark.
The benchmark measures real request and runtime behavior on the current host.

The demo benchmark is schema version 2. Its report keeps these clocks
separate: external HTTP latency, queue wait, dispatch gap, guest execution,
and internal total execution. CPU is intentionally allowed above 100% and is
also shown as equivalent logical CPUs; an unobserved sample is `null`/`N/A`,
not measured zero. `peak_active_lanes` comes from assignment events rather
than the sampler.
