# PitFast public alpha support matrix

PitFast is public alpha software. This page records observed paths, not a
promise that every framework or dependency is compatible.

## Observed working

These paths have been built and executed as WASI Components through PitFast:

- `wasi-http` and `wasi-cli` native contracts
- JavaScript Fetch applications through `javascript/fetch`
- Go `net/http` applications through `go/net-http`
- Python ASGI applications through `python/asgi` when dependencies are
  compatible with the current componentize-py runtime
- static output from React, Vue, Angular, Svelte, Solid, Preact, Next.js
  static export, Nuxt static generation, SvelteKit static output, and Astro
  static output through one `static-web` adapter
- already-built raw WASI Components

Framework names are detector hints and test evidence. They are not runtime
compatibility units. Any project that produces the same supported interface
can use the same generic adapter.

## Known limitations

- Node-oriented SSR and server modes that require a persistent Node process or
  Node-only APIs are not supported by the current WASI runtime path.
- FastAPI applications whose dependency graph includes the native
  `pydantic_core` CPython extension are blocked by the current componentize-py
  WASI runtime. The ASGI adapter itself remains generic.
- PHP, Ruby, JVM, .NET, BEAM, and arbitrary native Linux executable paths do
  not currently have a validated PitFast Component toolchain.
- The first public distribution target is Linux x86_64 in a glibc-compatible
  environment. Other operating systems and architectures are not claimed.
- Public alpha is not production HA or production security assurance.

When a project cannot run, use `pit doctor --verbose` for the compatibility
layer and `pit diagnostics --output pit-diagnostics.json` for a redacted issue
attachment. Review diagnostics before sharing them publicly.
