# Contributing to PitFast

PitFast is six independent repositories. There is intentionally no parent
Cargo workspace and no root Git repository. The coordinating public entry
point is `pit-cli`.

Before opening a change, run the quality gates for the repository you touched:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

`pit-cli` uses the equivalent non-workspace commands documented in its README.

Keep framework names out of runtime execution paths. Frameworks may appear in
detectors, fixtures, docs, and compatibility evidence; runtime behavior must
dispatch on generic application interfaces and WASI contracts. Do not add
containers, service ports, replicas, or host-process fallbacks.

Changes affecting a coordinated release must update
`release/pitfast-release.toml` for sibling repository revisions. The bundle
builder records the coordinating `pit-cli` commit after checkout, avoiding a
self-referential commit hash.
