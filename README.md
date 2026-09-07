# pit

pit is the PitFast developer CLI. It is a thin composition layer:

`pit call pit://service/path` sends a logical HTTP request to the local PitLane
listener without DNS resolution. PitLane selects the service while the guest
receives the original `/path`.

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

From any unambiguous supported project:

~~~bash
pit build
pit run
~~~

`pit build` selects the registered `LanguageBuilder` by explicit `--language`,
`pit.toml`, or deterministic auto-detection. Rust, Go, C, C++, JavaScript,
TypeScript, and Python currently pass real Component lifecycle tests. With
multiple possible build roots, detection fails instead of guessing. Use
`pit doctor languages` to inspect actual toolchain capability.

~~~bash
pit build --language rust
pit build --language go
pit build --language python
pit doctor languages
~~~

The selected builder validates the Component, writes `.pit/build/<name>.wasm`
and `.pit/artifact.json`, and participates in the same cache/fingerprint path.
Use `pit build --force` to rebuild and `pit build --debug` for a debug profile.
Use pit build --abi wasi-preview1 for the legacy core-module compatibility path.

## Runtime commands

~~~bash
pit run ./custom.wasm
pit run ./custom.wasm --concurrency 100 --timeout 2s --memory 64MiB --env MODE=production -- arg1 arg2
pit run
pit bench
pit bench ./custom.wasm
pit system
pit init --language rust
pit init --language go
pit init --language python
pit inspect
pit clean
pit call pit://service-a/hello
pit call --method POST --header 'Content-Type: application/json' \
  --body '{"hello":"world"}' pit://service-a/echo
pit push service-a:v1 --paddock-dir /tmp/pit-paddock
pit pull service-a:v1 --paddock-dir /tmp/pit-paddock
pit paddock list --paddock-dir /tmp/pit-paddock
pit paddock inspect service-a:v1 --paddock-dir /tmp/pit-paddock
pit deploy service-a service-a:v1 --paddock-dir /tmp/pit-paddock
pit deploy service-a service-a:v1 --paddock origin
pit deploy service-a sha256:<64-lowercase-hex> --artifact-store /path/to/artifacts
pit deploy service-a --local
pit service list
pit service inspect service-a
pit service history service-a
pit rollback service-a
pit undeploy service-a
~~~

Resource and service configuration can be inspected without printing secrets:

~~~bash
pit resource list
pit resource inspect main
pit resource check main
pit service inspect service-a
~~~

run, bench, and system use PitBox's existing local WASI Preview 1 and Preview 2 execution,
limits, scheduler, telemetry, and benchmark behavior. This CLI does not
implement Wasmtime, scheduling, or compilation.

pit run and pit bench without a path verify the manifest's artifact hash, size,
runtime ABI, and entrypoint before using PitBox. Raw commands such as
pit run ./custom.wasm remain available without a manifest. CLI execution flags
override pit.toml values, which override manifest execution defaults, which
override PitFast runtime defaults.

pit inspect prints and verifies a manifest without executing WASM. pit clean
removes only .pit/. pit init records the selected language in an existing
project and ensures .pit/ is in .gitignore; it does not generate a
framework-specific application template.

WASI Preview 2 command and `wasi:http/proxy` components are supported. Managed
artifacts are integrity-checked before execution; custom database WIT, PGlite,
TLS, and unrestricted external networking are not included. `pit call` uses
PitLane's local logical resolver; it does not resolve the service name with
DNS.
raw `pit run ./custom.wasm` remains available without a manifest.

The current development build uses relative path dependencies on ../pit-box and
../pit-crew. They can later be replaced with published or git dependencies.
Paddock uses the local filesystem backend by default; `PIT_PADDOCK_ROOT` or
`--paddock-dir` selects its content-addressed root. Paddock refs are mutable
name:tag pointers, while `sha256:` digests identify immutable artifact bytes.

Deployment resolves a ref once and sends the verified digest and local
digest-addressed artifact to PitLane's loopback-only control endpoint
(`127.0.0.1:7081` by default). The CLI does not edit deployment state files or
the live registry. `pit rollback` uses a historical digest directly; it does
not re-resolve the historical source tag. `pit undeploy` stops new routing but
keeps state history and artifacts.

Named Paddocks are configured in `pit.toml` (project configuration overrides
the optional user configuration at `$XDG_CONFIG_HOME/pit/pit.toml`):

```toml
[paddocks.local]
provider = "filesystem"
path = "/tmp/pit-paddock"

[paddocks.origin]
provider = "s3"
endpoint = "https://s3.example"
bucket = "pitfast-artifacts"
region = "auto"
access_key_env = "PIT_PADDOCK_ACCESS_KEY"
secret_key_env = "PIT_PADDOCK_SECRET_KEY"

[deployment]
default_paddock = "local"
```

`pit deploy --paddock origin` resolves a ref once, acquires the resulting
digest into the host LocalArtifactStore, and only then asks PitLane to prepare
and activate it. A missing digest is fetched by digest for rollback; historical
tags are never re-resolved. `--paddock-dir` is an explicit filesystem override,
useful for offline tests. Remote acquisition has a bounded 30-second timeout
and three attempts. Paddock is never contacted on the request hot path.

## Application interfaces and adoption

`pit init` is interface-first. It reports the language, application interface,
entrypoint, framework hint, and evidence used by the detector. A detector is a
convenience hint only; it is not required to build an application.

```bash
pit init --dry-run
pit init --language python --interface asgi --entry main:app
pit doctor
pit build --language python --interface asgi --entry main:app
```

The Python ASGI adapter bridges any compatible ASGI application to
`wasi:http/proxy`; the PitFast runtime does not know the framework name. A
project-local declarative adapter can be selected with
`--adapter ./pit-adapters/<name>` and can introduce a new interface without a
PitCrew source change. It may list generated source assets, but v0.11 does not
execute arbitrary downloaded native plugins.

Already-built Components bypass all source detection:

```bash
pit build --artifact ./dist/app.wasm --abi wasi-preview2 --world wasi:http/proxy
```

The artifact is still validated and enters the normal manifest, Paddock, and
deployment lifecycle.

## Language status

After build time the runtime is language-neutral: PitBox sees only the
manifest's WASI ABI/world and a prepared Module/Component. It never starts a
Node, Python, JVM, or .NET process for a request. JavaScript and Python runtime
support is embedded in their Components.

| Language | Status | Build-time path |
| --- | --- | --- |
| Rust | First-class | rustc/Cargo |
| Go | First-class | TinyGo/componentize-go |
| C | First-class | WASI SDK clang + wit-bindgen C ABI |
| C++ | First-class | WASI SDK clang++ + stable generated C ABI |
| JavaScript | First-class | Node + componentize-js |
| TypeScript | First-class | tsc + componentize-js |
| Python | First-class | componentize-py with embedded Python |
| C# | Blocked | available .NET workload is browser-WASM only; no standalone wasm32-wasi Component backend |
| Java | Blocked | no validated Java-to-WASI-Component toolchain in this environment |

The C++ fixture uses the generated stable C ABI because the current C++ guest
binding output has an upstream `std::expected`/forward-declaration issue; the
result is still a standard `wasi:http/proxy` Component. Python's fixture uses
the official componentize-py support package. These are adapter/toolchain
details and do not enter PitBox execution dispatch.

Known Python limitation: the current upstream componentize-py pre-initialized
CPython snapshot is not byte-for-byte deterministic across forced rebuilds.
Normal fingerprint cache reuse remains stable, but `pit build --force` may
produce a different Python artifact digest; PitFast reports this limitation
rather than treating the outputs as reproducible.
