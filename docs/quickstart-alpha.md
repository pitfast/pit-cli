# PitFast public alpha quickstart

PitFast is public alpha software for Linux x86_64 on glibc-compatible systems.
It keeps application workloads disposable and infrastructure shared; it does
not start one process, port, or replica per manifest service.

Install the published prerelease without cloning PitFast:

```bash
curl -fsSL https://raw.githubusercontent.com/pitfast/pit-cli/v0.14.0-alpha.1/scripts/install-alpha.sh \
  | bash -s -- --version 0.14.0-alpha.1
export PATH="$HOME/.local/bin:$PATH"
pit --version
pit doctor system
```

For a local source build, use:

Install a locally built alpha bundle without root:

```bash
./release/build-alpha-bundle.sh --output ./release/dist
./scripts/install-alpha.sh \
  --archive ./release/dist/pitfast-0.14.0-alpha.1-x86_64-unknown-linux-gnu.tar.gz \
  --version 0.14.0-alpha.1 \
  --prefix "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"
pit --version
```

The bundle is Linux x86_64 alpha software. Its archive checksum is mandatory;
the installer accepts a local archive, a URL plus its `.sha256` companion, or
an explicit `--checksum` file. No hosted release URL is claimed until one is
actually published. The versioned install lives under
`~/.local/share/pitfast/versions/`; rerunning the installer with another
version atomically switches the `pit` symlink. Remove only the managed files
with `scripts/install-alpha.sh --uninstall --prefix "$HOME/.local"`.

For source-checkout development, `./scripts/install-dev.sh` remains a CLI-only
installer and does not provide the coherent local runtime bundle.

From a compatible project, the normal path is:

```bash
pit init
pit doctor
pit up
```

`pit up` builds static frontend output or a supported application interface,
prepares the artifact, and activates one coherent application release. It does
not start one process or port per service. A packaged `pit` automatically
starts one detached local PitLane infrastructure daemon when the default local
endpoint is unavailable; source-checkout users may continue to provide an
explicit `--control-endpoint`. Use `pit doctor system` to inspect the host and
optional build toolchains. Use `pit up --timings` when diagnosing build,
artifact, preparation, or activation time. Restart restores the authoritative
active release and route snapshot before bounded artifact prewarming; the
first request may pay deferred preparation latency.

For a project with one `.pit` file, its filename is free. With multiple files,
`app.pit` is the only automatic tie-break; otherwise pass `--file` explicitly.
