# PitFast

PitFast is a WASM-native, execution-first application platform. Applications
do not own replicas, service ports, or persistent service processes; PitFast
keeps infrastructure ready and executes workloads only when work exists.

**Public alpha:** expect breaking changes, incomplete compatibility, and no
production SLA or HA guarantee.

## Install

The public alpha distribution targets Linux x86_64 in a glibc-compatible
environment. Install the published, checksum-verified prerelease:

```bash
curl -fsSL https://raw.githubusercontent.com/pitfast/pit-cli/v0.14.0-alpha.1/scripts/install-alpha.sh \
  | bash -s -- --version 0.14.0-alpha.1
export PATH="$HOME/.local/bin:$PATH"
```

For a local source build, use the bundle instructions in
[`docs/quickstart-alpha.md`](docs/quickstart-alpha.md).

After installation, verify the software and host:

```bash
pit --version
pit doctor system
```

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

## Pit Manifest and one-command workflow

The primary application configuration is a Pit Manifest with the `.pit`
extension. It is TOML-compatible, but its schema and meaning belong to
PitFast rather than to the legacy project build configuration. `pit init`
creates the conventional `app.pit`; any filename is valid, so a lone
`backend.pit` or `customer-acme.pit` works identically.

```toml
schema = 1
name = "shop"

[services.api]
build = "./api"
interface = "net-http"
entry = "app:Handler"

[services.catalog]
build = "./catalog"
interface = "asgi"
entry = "main:app"

[routes]
"/api" = "api"
"/catalog" = "catalog"
```

`pit up` resolves, validates, builds, and activates every service in the
manifest through the existing PitLane deployment primitives. It does not
start persistent application processes or allocate service ports; services
remain logical `ServiceId` identities whose immutable artifacts are executed
on shared PitFast infrastructure.

Manifest selection is deterministic and shared by `pit up`, `pit build`,
`pit doctor`, `pit deploy`, and `pit config show`:

1. `--file/-f` always wins and must name an existing `.pit` file.
2. Otherwise only `./*.pit` in the current directory is considered.
3. One file is selected regardless of its name.
4. Multiple files select `app.pit` when present; otherwise the command fails
   with `AmbiguousManifest` and lists the files.

Paths such as `build = "../api"` are resolved relative to the selected
manifest, never relative to a caller's process directory or a parent
directory. `pit config show -f path/to/prod.pit` prints the normalized plan.

The legacy `pit.toml` remains a per-project build/defaults compatibility
layer for existing single-project workflows. A manifest can override those
defaults per service; new application projects need only one `.pit` file.

```bash
pit init
pit doctor
pit up
pit up --file prod.pit
pit build --file app.pit api
pit deploy --file prod.pit
pit config show --file app.pit
pit diagnostics --output pit-diagnostics.json
```

Resources in a manifest are external capability bindings, not PitFast
workloads. Routes target logical services, and service-to-service calls keep
using `pit://service-id/...`; manifests contain no container, replica,
network, or service-port model.

## Legacy single-project workflow

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

`pit doctor` is intended to answer whether an existing repository can cross
the WASI boundary before an expensive build. It distinguishes confirmed
blockers from potential or unknown findings. Use `--verbose` for transitive
dependency paths and `--json` for CI:

```bash
pit doctor --verbose
pit doctor --json
```

For a safe support attachment, run:

```bash
pit diagnostics --output pit-diagnostics.json
```

The JSON contains host, version, toolchain, manifest structure, and interface
metadata, but not environment values, credentials, authorization headers, or
raw logs. Review it before sharing.

The doctor does not equate framework recognition with compatibility. For
example, FastAPI is detected as an ASGI hint, then its `fastapi → pydantic →
pydantic_core` native CPython/Linux wheel is reported as a confirmed blocker
for the current componentize-py WASI runtime. No host Python process is
started.

The Python ASGI adapter bridges any compatible ASGI application to
`wasi:http/proxy`; the PitFast runtime does not know the framework name. A
project-local declarative adapter can be selected with
`--adapter ./pit-adapters/<name>` and can introduce a new interface without a
PitCrew source change. It may list generated source assets, but v0.11 does not
execute arbitrary downloaded native plugins.

The same interface-first rule applies to JavaScript/TypeScript Fetch handlers
and Go `net/http` handlers. Fetch projects use `--interface fetch --entry
main:fetch`; Go projects use `--interface net-http --entry app:Handler`. Their
generated bridges are build-time files under `.pit/generated`; application
source is not rewritten and applications do not bind service listeners.

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
# Pit Manifest lifecycle

`pit up` resolves one `*.pit` manifest, builds and prepares every service,
then publishes one coherent application release. A sole manifest may use any
filename; when several are present `app.pit` is the conventional default and
otherwise `--file` is required. Paths in a manifest are relative to that
manifest.

The release contains logical service-to-`ArtifactDigest` mappings, routes, and
resource bindings. It does not create processes, replicas, ports, or a
container network. Inspect a side-effect-free plan with `pit plan`, list
history with `pit releases APP`, and roll back a whole application with
`pit rollback --application APP`.
