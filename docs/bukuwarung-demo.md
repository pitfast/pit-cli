# BukuWarung technical demo runbook

This is a 15–20 minute, local-first technical discussion. It demonstrates
that an ApplicationRelease can remain deployed and routable while guest
executions are absent between requests. Pit Web shows durable/configured
state, PitLane makes logical routing decisions, and Cockpit shows ephemeral
execution.

## Prepare and start

From the demo checkout:

```sh
cd /home/ecenso/pitfast/pit-cli/examples/pitstop-demo
./scripts/demo-preflight.sh
./scripts/demo-build.sh
./scripts/demo-start.sh
./scripts/demo-reset.sh
```

The build is the only step expected to need package/toolchain access. After
it succeeds, the local demo uses the checked-out binaries, cached artifacts,
and local fixture; it does not need Docker, nginx, a database, or an
external API.

Recommended layout:

- Terminal 1: PitLane log printed by `demo-start.sh`.
- Terminal 2: `pit cockpit`.
- Terminal 3: demo commands.
- Browser: `http://127.0.0.1:7080/__pit/`.

No tmux session is required.

The demo startup wrapper binds HTTP and control to `0.0.0.0` so a browser on
the same trusted LAN can use the LAN address printed by `demo-start.sh`, for
example `http://192.168.x.x:7080/__pit/`. The control API is exposed on port
7081 in this explicit demo mode and includes mutation endpoints; use this only
on a trusted isolated network. Standalone PitLane remains loopback-only unless
`--allow-public-control` is explicitly supplied.

## 15–20 minute flow

1. Ask how many services, runtimes, minimum replicas, and bursty workloads
   the team operates. A useful question is: “If all application traffic
   disappeared for one hour, how much application compute would still be
   running?”
2. Open Pit Web. Show `pitstop-demo`, its `web`, `orders`, and `users`
   services, active release, routes, artifacts, and Circuit/Garage inventory.
   Point out that running executions are zero.
3. Open Cockpit and show an empty queue, free Lanes, and zero active Stores.
4. Run `./scripts/demo-live.sh`. Explain request → Lane → execution → exit.
5. Run `./scripts/demo-burst.sh medium`. Point out shared Grid occupancy and
   queueing when capacity is busy. Wait for zero running executions and zero
   active Stores.
6. Optionally run `./scripts/demo-routing-short.sh` to show one stable →
   candidate → stable release switch through the same listener. Use the full
   `demo-routing.sh` only when the audience wants the policy matrix.
7. Return to Pit Web and show that the application, release, and routes are
   still present after guest work has disappeared.
8. Ask: “Which BukuWarung workload would be the fairest one to benchmark
   against the way you run it today?”

The default hero flow is stable routing. Canary, deterministic A/B, header
override, and shadow traffic remain regression-tested capabilities, not part
of the primary narrative unless requested.

For the technical appendix, the focused native-capability proof is:

```sh
./scripts/demo-nested-release.sh
```

It uses the existing `pit://users` logical invocation path and reports BLUE →
BLUE, GREEN → GREEN, and an in-flight BLUE request that remains BLUE after the
new default switches to GREEN. It does not use service DNS.

## Safe talk track

“PitFast separates deployment persistence from execution residency. The
application remains deployed and routable, but that does not require an
application process to remain alive waiting for traffic. PitFast schedules
runnable executions onto shared capacity.”

Say “zero persistent guest application-process residency when idle”; do not
say “zero RAM”. Infrastructure and prepared artifacts may remain resident.
Say “microsecond-scale dispatch/scheduler overhead for ready work where
measured”; do not claim microsecond cold starts.

The target is persistent workload residency and replica-oriented capacity
allocation, not the container format itself. PitFast explores an
execution-first alternative for suitable request-driven/stateless workloads;
it does not claim to replace Kubernetes universally or to be production-ready.

## Short FAQ

**Doesn’t Kubernetes/Knative already scale to zero?** Yes. PitFast’s
distinction is that execution scheduling is the native model rather than
representing zero as a scaling state of a replica-oriented deployment.

**Aren’t containers not necessarily heavy?** Correct. The target is
persistent process/workload residency and capacity allocation, not the
container format.

**Why WASM?** Portable executable artifacts, runtime isolation boundaries,
fast preparation/reuse, and a model that schedules executions without
service-owned processes and ports.

**What is the fair comparison?** One low-risk, stateless, bursty workload
with a known latency/concurrency target—not a production migration promise.

## Warmup and recovery

Ten to fifteen minutes before the meeting: plug in power, choose a sleep
policy, close heavy applications, run preflight/build/start, perform one
rehearsal, run `demo-reset.sh`, and leave Pit Web plus Cockpit open at idle.
Do not run a heavy benchmark immediately before the call if it could cause
thermal throttling.

For the full automated rehearsal:

```sh
./scripts/demo-rehearsal.sh
```

It writes `artifacts/demo/rehearsal-latest.{json,md}` and fallback evidence.
The independent repeat campaign is intentionally run from a terminal so each
attempt is visible:

```sh
for i in $(seq 1 10); do
  echo "rehearsal $i"
  ./scripts/demo-rehearsal.sh || break
done
```

Use `./scripts/demo-reset.sh` between manual experiments and
`./scripts/demo-stop.sh` after the session. See
[`bukuwarung-demo-recovery.md`](bukuwarung-demo-recovery.md) for symptoms and
safe recovery commands.
