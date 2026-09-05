# pit

pit is the PitFast developer CLI. It is a thin composition layer:

~~~text
pit build → PitCrew
pit run   → PitBox
pit bench → PitBox
pit system → PitBox
~~~

## Primary workflow

From a Rust binary project:

~~~bash
pit build
pit run
~~~

pit build detects the Cargo project and its binary target, invokes the Rust
WASI Preview 1 builder, validates the generated module, and writes
.pit/build/<name>.wasm plus .pit/artifact.json. With multiple binaries, select
one with pit build --bin <name>. Use pit build --debug for a debug profile.

## Runtime commands

~~~bash
pit run ./custom.wasm
pit run ./custom.wasm --concurrency 100 --timeout 2s --memory 64MiB --env MODE=production -- arg1 arg2
pit run
pit bench
pit bench ./custom.wasm
pit system
~~~

run, bench, and system use PitBox's existing local WASI Preview 1 execution,
limits, scheduler, telemetry, and benchmark behavior. This CLI does not
implement Wasmtime, scheduling, or compilation.

The current development build uses relative path dependencies on ../pit-box and
../pit-crew. They can later be replaced with published or git dependencies.

