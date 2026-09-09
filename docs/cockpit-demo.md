# PitFast Cockpit Demo Alpha

This is a 15–20 minute, local, read-only demonstration of the PitFast
execution model. It uses one application release with a static frontend and
two logical API services plus a static frontend. The application has no database, Docker, nginx,
service-owned ports, or replicas.

## What the demo shows

PitFast keeps the application release and infrastructure ready. A service is
deployed even when no guest execution is resident. When requests arrive,
PitLane routes them logically and PitBox assigns runnable work to shared
Grid/Lane capacity.

The short explanation is:

> Deployed does not mean running. PitFast keeps the release and
> infrastructure ready, then assigns execution capacity only when runnable
> work arrives.

## Setup from a source checkout

Build the two local binaries, then start one local PitLane instance. These
commands assume the six repositories are siblings under the same directory:

```sh
cd /home/ecenso/pitfast/pit-lane
cargo build --workspace --release
cd /home/ecenso/pitfast/pit-cli
cargo build --release

PIT_STATE="$(mktemp -d /tmp/pitfast-cockpit-state.XXXXXX)"
PIT_ARTIFACTS="$(mktemp -d /tmp/pitfast-cockpit-artifacts.XXXXXX)"
./../pit-lane/target/release/pit-lane \
  --listen 127.0.0.1:7080 \
  --control-listen 127.0.0.1:7081 \
  --state-dir "$PIT_STATE" \
  --artifact-store "$PIT_ARTIFACTS"
```

In a second terminal, deploy the demo. `pit up` selects the only
`*.pit` file, regardless of its filename.

```sh
cd /home/ecenso/pitfast/pit-cli/examples/pitstop-demo
/home/ecenso/pitfast/pit-cli/target/release/pit up --timings
```

For an installed bundle, use the installed `pit` entry point and its bundled
local infrastructure according to the release quickstart; no PitFast source
checkout is required by the user-facing flow.

## Run the Cockpit

In a third terminal:

```sh
cd /home/ecenso/pitfast/pit-cli/examples/pitstop-demo
/home/ecenso/pitfast/pit-cli/target/release/pit cockpit
```

The default view polls the loopback read-only endpoint every 100 ms. Press
`q` or `Esc` to exit, and `p` or Space to pause. A slower refresh can be
selected with `--refresh-ms 200`.

The top half reads `QUEUE → PIT ENTRY → LANES → EXIT`: queued requests are
yellow tokens, occupied lanes are red, free lanes are dim, and recent
completed/failed executions appear on the exit rail. The lower panels show
real service readiness, queue/running/completed counts, timing detail, CPU,
RSS, active guest executions, and prepared artifact state. `peak-lanes` is an
event-derived scheduler counter, so a short execution is counted even if it
finishes between two TUI polls.

## Live walkthrough

With the Cockpit running:

```sh
./scripts/demo-live.sh
```

This checks the frontend and both APIs, then sends parallel medium CPU work to
orders and users. It is intentionally small enough for a narrated demo.

For a visible burst:

```sh
./scripts/demo-burst.sh medium
./scripts/demo-burst.sh burst
```

The medium profile sends 24 requests at concurrency 8. The burst profile
sends 64 larger CPU requests at concurrency 32. Profiles are bounded and
finish without leaving application processes resident.

## Benchmark workflow

Run from the demo directory after `pit up`:

```sh
./scripts/bench-demo.sh
```

The script records real external HTTP percentiles and joins them with
completed Cockpit telemetry for queue wait, dispatch gap, guest execution,
internal total execution, event-derived active-lane peak, CPU, and RSS. It
writes:

```text
artifacts/bench/summary.json
artifacts/bench/summary.md
```

The scenarios are idle, single request, mixed light concurrency, burst, and a
medium CPU mix. The report records host information and explicitly does not
claim OS page-cache coldness or zero infrastructure memory.

## BukuWarung talk track

1. “These three services are one ApplicationRelease, not three host daemons.”
2. “They remain deployed while the Cockpit shows zero running guest work.”
3. “Requests form a queue, enter the Pit Lane, and use whichever shared Lane
   is free.”
4. “A red Lane is runnable work in execution; after completion the token exits
   and the Lane becomes available again.”
5. “The target problem is persistent application residency. This is not a
   claim that containers or Kubernetes are universally wrong.”
6. “Kubernetes commonly schedules instances/replicas; PitFast schedules
   runnable executions against shared capacity.”

Safe claims: zero persistent guest application-process residency while idle,
logical service routing without service-owned ports, and measured scheduler /
execution timings from this host. Do not claim zero RAM, universal framework
support, or a production HA guarantee.

## Design-partner questions

- Which short-lived business operations are currently over-provisioned because
  their process must stay resident?
- Which burst patterns matter most: order writes, catalog reads, user actions,
  or scheduled jobs?
- What latency and concurrency SLOs should a shared execution pool expose?
- Which language/application interfaces would be the first useful validation?
- Which observability fields would an on-call engineer need before adopting a
  disposable-execution model?

## Demo boundaries

The Cockpit is read-only and local. It is not a control plane, deployment UI,
distributed tracing system, or Prometheus replacement. PitFast infrastructure
may remain resident; the claim is specifically about guest application
execution residency. The demo uses already-supported generic interfaces:
static-web, Go `net/http`, and Python ASGI. The frontend is a plain JavaScript
static bundle, so the demo crosses three supported application styles without
requiring a fourth service.
