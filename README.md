# pit

pit is the PitFast developer CLI. It is a thin composition layer:

~~~text
pit build → PitCrew
pit run   → PitBox
pit bench → PitBox
pit system → PitBox
~~~

The project contract has three layers:

* pit.toml contains optional user intent such as a selected binary and
  execution defaults.
* .pit/artifact.json contains generated, versioned artifact metadata.
* .pit/build/*.wasm contains generated executable output.

## Primary workflow

From a Rust binary project:

~~~bash
pit build
pit run
~~~

pit build detects the Cargo project and its binary target, invokes the Rust
WASI Preview 1 builder, validates the generated module, and writes
.pit/build/<name>.wasm plus .pit/artifact.json. With multiple binaries, select
one with pit build --bin <name>. Repeated builds reuse a valid artifact when
the source/build fingerprint is unchanged. Use pit build --force to rebuild.
Use pit build --debug for a debug profile.

## Runtime commands

~~~bash
pit run ./custom.wasm
pit run ./custom.wasm --concurrency 100 --timeout 2s --memory 64MiB --env MODE=production -- arg1 arg2
pit run
pit bench
pit bench ./custom.wasm
pit system
pit init
pit inspect
pit clean
~~~

run, bench, and system use PitBox's existing local WASI Preview 1 execution,
limits, scheduler, telemetry, and benchmark behavior. This CLI does not
implement Wasmtime, scheduling, or compilation.

pit run and pit bench without a path verify the manifest's artifact hash, size,
runtime ABI, and entrypoint before using PitBox. Raw commands such as
pit run ./custom.wasm remain available without a manifest. CLI execution flags
override pit.toml values, which override manifest execution defaults, which
override PitFast runtime defaults.

pit inspect prints and verifies a manifest without executing WASM. pit clean
removes only .pit/. pit init creates a small pit.toml for an existing Cargo
project and ensures .pit/ is in .gitignore.

The current development build uses relative path dependencies on ../pit-box and
../pit-crew. They can later be replaced with published or git dependencies.
