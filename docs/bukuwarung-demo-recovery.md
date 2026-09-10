# BukuWarung demo recovery

All commands below are scoped to the demo-owned process and state directory.
They do not kill arbitrary `pit` or Rust processes.

## PitLane is not responding

Check `curl -fsS http://127.0.0.1:7081/v1/runtime`, then inspect the log path
printed by `demo-start.sh`. If the PID belongs to this demo, run
`./scripts/demo-stop.sh` followed by `./scripts/demo-start.sh`. If another
process owns the port, leave it alone and either use that known stack or move
the demo to an explicitly configured endpoint.

## Port 7080 or 7081 is occupied

Run `ss -ltnp | grep -E ':7080|:7081'` for diagnosis. Do not kill the reported
process automatically. Stop the known owner through its own command, or set
`PITFAST_HTTP_ENDPOINT`, `PITFAST_CONTROL_ENDPOINT`, and matching listener
arguments for a separately prepared demo environment.

## Pit Web is 404 or assets fail

Check `curl -i http://127.0.0.1:7080/__pit/` and the JS/CSS paths shown by the
HTML. Run `./scripts/demo-start.sh` again to re-activate the bundled static
application. If the artifact is missing, run `./scripts/demo-build.sh` before
starting; do not build during the meeting.

For a LAN browser, use the `LAN Pit Web` URL printed by `demo-start.sh`. Do
not expose the control port on an untrusted network; public demo mode is an
explicit convenience mode, not an authentication layer.

## Management or Cockpit API fails

Run:

```sh
curl -fsS http://127.0.0.1:7081/v1/management/snapshot
curl -fsS http://127.0.0.1:7081/v1/cockpit/snapshot
```

If the listener is healthy, retry `demo-reset.sh`. If it remains unhealthy,
save `artifacts/demo/runtime/logs/pit-lane.log`, stop/start the demo, and use
the fallback rehearsal evidence.

## Burst is stuck or idle does not return

Wait 30 seconds, then inspect the burst output and snapshot. Run
`./scripts/demo-reset.sh`; it only clears the demo route policy and waits for
queue/running/active Stores to reach zero. If recovery fails, run
`./scripts/demo-stop.sh`, `./scripts/demo-start.sh`, and `./scripts/demo-reset.sh`.

## Unexpected route state

Run `pit route list --control-endpoint http://127.0.0.1:7081`, then run
`./scripts/demo-reset.sh`. The reset returns the demo route to active-release
default routing without deleting build caches.

## Browser failure

Use the direct URL with a normal browser, or validate the same HTML, assets,
and JSON endpoints with curl. Do not claim browser validation if no browser
could be launched. The captured fallback files under `artifacts/demo/fallback`
are the honest backup evidence from the last successful rehearsal.

## Artifact/build failure

Run `./scripts/demo-preflight.sh`, check the relevant build log, and run
`./scripts/demo-build.sh` before the meeting. The fixture has no database or
internet API dependency; source/toolchain errors are the likely cause.
