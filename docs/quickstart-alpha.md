# PitFast developer alpha quickstart

Install the local alpha CLI without root:

```bash
./scripts/install-dev.sh --prefix "$HOME/.local/bin"
export PATH="$HOME/.local/bin:$PATH"
pit --version
```

From a compatible project, the normal path is:

```bash
pit init
pit doctor
pit up
```

`pit up` builds static frontend output or a supported application interface,
prepares the artifact, and activates one coherent application release. It does
not start one process or port per service. Use `pit doctor system` to inspect
the host and optional build toolchains. Use `pit up --timings` when diagnosing
build, artifact, preparation, or activation time.

For a project with one `.pit` file, its filename is free. With multiple files,
`app.pit` is the only automatic tie-break; otherwise pass `--file` explicitly.
