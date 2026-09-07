# PitFast v0.12.2 `pit up` latency audit

These are controlled local measurements on Debian GNU/Linux 13.6,
x86_64, kernel `6.12.101+deb13-amd64`. Debug and release binaries were
measured separately. `COLD` means a forced/new artifact preparation path;
`WARM` means the persistent compiled artifact exists after a restart; and
`HOT/repeat` means the same PitLane process has already prepared the artifact.
These are PitFast-cache states, not claims about a cold OS page cache.

## Application matrix

| Services | Debug COLD | Debug WARM/repeat | Release COLD | Release HOT/repeat |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 8.89 s | 1.23 s | 7.57 s | 0.215 s |
| 4 | 29.26 s | 4.56 s | 24.01 s | 0.574 s |
| 10 | 45.83 s | 12.16 s | 33.00 s | 2.12 s |

The ten-service fixture deliberately reused one artifact for nine services;
its release cold path therefore recorded one cold compile and nine prepared
cache hits, rather than ten independent component compiles.

## Release stage samples

| Services | Build | Local prepare | Activation HTTP | PitBox preparation | Prepared hits | Cold compiles |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 cold | 2.731 s | 43 ms | 4.789 s | 4.777 s | 0 | 1 |
| 1 repeat | 162 ms | 40 ms | 12 ms | — | 1 | 0 |
| 4 cold | 9.241 s | 168 ms | 14.603 s | 14.556 s | 0 | 4 |
| 4 repeat | 376 ms | 152 ms | 44 ms | — | 4 | 0 |
| 10 cold | 27.833 s | 386 ms | 4.778 s | 4.664 s | 9 | 1 |
| 10 repeat | 1.623 s | 383 ms | 113 ms | — | 10 | 0 |

The timing endpoint now exposes validation, artifact verification,
preparation, persistence, snapshot publication, prepared hits, cold compiles,
and warm restores. The CLI renders these with `pit up --timings`.

## Root cause of the previous multi-minute observation

The multi-minute observation was a debug PitLane restart replaying the
persisted active application and restoring/preparing its components. It was
not an unchanged release activation in the same process:

| Scenario | Wall time |
| --- | ---: |
| Debug persisted-state restart | 195.710 s |
| Release persisted-state restart | 15.283 s |
| Four-service debug forced preparation | 29.26 s |
| Four-service debug repeat | 4.56 s |
| Four-service release repeat | 0.574 s |

The debug restart also reached roughly 750% CPU and about 927 MB RSS on this
host. The evidence points to debug Wasmtime/component restoration and repeated
startup preparation, not manifest discovery or a hidden Lane wait. Repeated
release `pit up` now avoids rebuilds and compilation: the second run reports
prepared-cache hits and zero cold compiles.

## Preparation and Lane boundary

Artifact verification and `PreparedArtifact` construction remain before
execution admission. No execution Lane is held while Paddock acquisition,
compiled restore, compilation, or preparation occurs. Direct application
release preparation remains sequential in this milestone; the measured cold
cost is therefore visible rather than hidden. No speculative parallel
preparation default was selected without a bounded A/B result.

## Fresh alpha path

The local source installer was exercised into a temporary user prefix:

```text
./scripts/install-dev.sh --prefix <temporary-prefix>/bin
pit --version
pit doctor system
pit init
pit doctor
pit up
```

The fresh React project reached a real HTTP response from PitFast. The
installer does not silently install Node; `pit doctor system` reports missing
optional build tools, while the project-specific doctor explains which tool is
required.
