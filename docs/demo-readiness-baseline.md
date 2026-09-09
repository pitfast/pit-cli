# BukuWarung Demo Readiness Baseline

Captured before the final rehearsal hardening work on 2026-09-09.

## Current stack

- Six independent repositories remain in place; there is no root Git
  repository, Cargo workspace, or umbrella manifest.
- `pit web` is a read-only static-web PitFast application at
  `http://127.0.0.1:7080/__pit/`.
- `pit cockpit` is a read-only live execution view backed by
  `/v1/cockpit/snapshot`.
- `pitstop-demo` contains `web` (JavaScript static-web), `orders` (Go
  `net/http`), and `users` (Python ASGI), with no database or external API.

## Existing evidence

Cockpit already exposes real queue, Lane, execution, release, readiness,
CPU/RSS, Store, and event-derived peak-Lane data. Existing live, burst,
benchmark, and routing scripts are retained. Pit Web HTTP/static/API checks
were previously validated against real local state.

The runtime's native logical invocation contract is the `pitfast:service`
capability, using `pit://service/path`. The host passes the captured
application and release context into the invocation request. Service DNS is
not part of the proof.

## Remaining proof gap

The existing release-routing proof exercised release selection and in-flight
switching, but did not have a fresh operator-facing fixture that proves a
same-application nested logical invocation remains on the captured release.
This milestone adds a focused regression/conformance proof for that contract
and records any limitation if a guest-language fixture cannot exercise the
native capability without adding an unrelated service.

## Operator gaps

The demo previously relied on manually starting PitLane and manually
remembering the cleanup sequence. This milestone adds deterministic
preflight, build, start, reset, stop, rehearsal, and recovery documentation.
All runtime state is isolated under the demo's ignored artifacts directory by
the new scripts, and stop/reset only operate on state owned by those scripts.

## Reproduced telemetry regression and fix

During rehearsal, management and Cockpit probes intermittently exceeded the
preflight timeout while the control listener itself remained reachable. The
cause was in the runtime readiness query: `warm_digests` read and validated
every compiled `.cwasm` payload on every snapshot. With a multi-gigabyte local
cache, this made a read-only status request perform a full cache scan. The
runtime fix uses cache metadata and file size for readiness instead of reading
compiled payload bytes; a regression test covers a 256 MiB sparse cache entry.
After rebuilding and restarting PitLane, 20 consecutive management/Cockpit
probe pairs completed successfully, with management snapshots around 14–15 ms
and Cockpit snapshots around 4–5 ms.

The authoritative lane peak remains event-derived. A short execution may not
appear in a sampled `running` field, but it still contributes to
`peak_active_lanes` recorded at assignment time.

## Tooling baseline

Rust, Go, Python, Node/npm/pnpm, and Playwright's CLI are available. No system
Chromium, Chrome, or Firefox executable was present at baseline; browser
validation is therefore attempted user-locally and is reported as skipped or
blocked if the required browser/runtime cannot be launched.
