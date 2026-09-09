# Pit Web v0.1

Pit Web is PitFast's local, read-only management console. It answers “what
exists, what is deployed, what is stored, and how is the local system
configured?” It is deliberately separate from Cockpit, which answers “what is
happening right now?”

## Start

From a PitFast source checkout, build the frontend and the two local binaries:

```sh
cd pit-cli/apps/pit-web
npm ci
npm run typecheck
npm run build
cd ../..
cargo build --release
cd examples/pitstop-demo
../../target/release/pit up
../../target/release/pit web --no-open
```

`pit web` prints and, by default, opens:

```text
http://127.0.0.1:7080/__pit/
```

The command materializes the checked-in Vite output into a temporary static
project, then uses the normal `pit up` path. The resulting Pit Web app is a
static-web WASI Component executed through ApplicationRelease, PitLane, and
PitBox. Node is build-time only; there is no Node production server, nginx,
Docker dependency, dedicated app port, or replica.

## Pages

- Overview: local summary and the Pit Web/Cockpit boundary.
- Applications: active application identity, services, and routes.
- Releases: immutable ApplicationRelease history and active generation.
- Routes: logical PitLane Host + Path selectors, release policies, stable and
  candidate release IDs, weights, stickiness headers, and shadow status.
- Paddock: artifact identities, configured object namespaces/objects, and
  backend capabilities when a grant is attached.
- Circuit: Garage inventory and capacity when Circuit is configured.
- System: safe local runtime facts.

The browser reads one aggregated `GET /v1/management/snapshot` response from
the loopback control listener at `127.0.0.1:7081`. The endpoint is read-only,
versioned (`schema: 1`), and composed from existing PitLane,
ApplicationRelease, artifact, Paddock, and Circuit authorities. Browser state
is only presentation state; no durable truth is written to localStorage or
frontend files.

## Security and boundary

The control listener remains loopback-only by default. The management response
contains logical identifiers and safe runtime facts, never environment values,
credentials, authorization headers, private keys, or arbitrary host paths.

Pit Web manages/inspects durable state. Cockpit observes queue, lanes,
executions, timing, readiness, and CPU/RSS. Pit Web has no deploy, rollback,
delete, ref-move, GC, or topology mutation controls in v0.1; use the CLI for
mutations.

Routing policy changes are also CLI-owned in this alpha. Use `pit route list`
and the `pit route stable`, `blue-green`, `canary`, `ab`, `header`, `shadow`,
and `clear` commands to operate the existing PitLane authority; Pit Web only
displays the resulting RouteSnapshot.

For the combined demo:

```sh
pit web
# in another terminal:
pit cockpit
# in the demo directory:
./scripts/demo-burst.sh medium
```

Pit Web continues to show the deployed app and active release after the burst
ends, while Cockpit returns to zero running guest executions. That is the
intended distinction: deployment persists; execution does not have to.
