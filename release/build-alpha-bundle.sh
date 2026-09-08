#!/usr/bin/env bash
set -euo pipefail

version="0.13.0-alpha.1"
output_dir=""
skip_build=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) version="${2:?--version requires a value}"; shift 2 ;;
    --output) output_dir="${2:?--output requires a directory}"; shift 2 ;;
    --skip-build) skip_build=1; shift ;;
    --help|-h)
      echo "Usage: $0 [--version VERSION] [--output DIRECTORY] [--skip-build]"
      exit 0
      ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cli_dir="$(cd -- "$script_dir/.." && pwd)"
root_dir="$(cd -- "$cli_dir/.." && pwd)"
output_dir="${output_dir:-$cli_dir/release/dist}"
target="$(rustc -vV | sed -n 's/^host: //p')"
[[ "$target" == "x86_64-unknown-linux-gnu" ]] || {
  echo "v0.13 alpha bundle currently requires x86_64-unknown-linux-gnu; found $target" >&2
  exit 1
}

if [[ "$skip_build" -eq 0 ]]; then
  cargo build --manifest-path "$cli_dir/Cargo.toml" --release
  cargo build --manifest-path "$root_dir/pit-lane/Cargo.toml" --workspace --release
  cargo build --manifest-path "$root_dir/pit-box/Cargo.toml" --workspace --release
  cargo build --manifest-path "$root_dir/pit-circuit/Cargo.toml" --workspace --release
fi

stage="$(mktemp -d "${TMPDIR:-/tmp}/pitfast-bundle.XXXXXX")"
trap 'rm -rf -- "$stage"' EXIT
mkdir -p "$stage/bin" "$stage/share"
declare -A binaries=(
  [pit]="$cli_dir/target/release/pit"
  [pit-lane]="$root_dir/pit-lane/target/release/pit-lane"
  [pit-node]="$root_dir/pit-box/target/release/pit-node"
  [pit-garage]="$root_dir/pit-box/target/release/pit-garage"
  [pit-circuit]="$root_dir/pit-circuit/target/release/pit-circuit"
)
for name in "${!binaries[@]}"; do
  source="${binaries[$name]}"
  [[ -x "$source" ]] || { echo "missing release binary: $source" >&2; exit 1; }
  install -m 0755 "$source" "$stage/bin/$name"
done

export PITFAST_BUNDLE_VERSION="$version"
export PITFAST_BUNDLE_TARGET="$target"
export PITFAST_STAGE="$stage"
export PITFAST_ROOT="$root_dir"
python3 - <<'PY'
import hashlib
import json
import os
import pathlib
import subprocess
import time

root = pathlib.Path(os.environ["PITFAST_ROOT"])
stage = pathlib.Path(os.environ["PITFAST_STAGE"])
repos = {
    "pit-box": root / "pit-box",
    "pit-crew": root / "pit-crew",
    "pit-cli": root / "pit-cli",
    "pit-lane": root / "pit-lane",
    "pit-paddock": root / "pit-paddock",
    "pit-circuit": root / "pit-circuit",
}
commits = {
    name: subprocess.check_output(
        ["git", "-C", str(path), "rev-parse", "HEAD"], text=True
    ).strip()
    for name, path in repos.items()
}
binaries = {}
for path in sorted((stage / "bin").iterdir()):
    data = path.read_bytes()
    binaries[path.name] = {
        "sha256": hashlib.sha256(data).hexdigest(),
        "size_bytes": len(data),
    }
manifest = {
    "schema": 1,
    "version": os.environ["PITFAST_BUNDLE_VERSION"],
    "target": os.environ["PITFAST_BUNDLE_TARGET"],
    "os": "linux",
    "architecture": "x86_64",
    "created_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "repositories": commits,
    "runtime": {"wasi": "preview2", "wasmtime": "39.0"},
    "binaries": binaries,
    "roles": {
        "local_runtime": ["pit", "pit-lane"],
        "distributed_runtime": [
            "pit", "pit-lane", "pit-node", "pit-garage", "pit-circuit"
        ],
    },
}
(stage / "share/release-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
PY

mkdir -p -- "$output_dir"
archive="$output_dir/pitfast-${version}-${target}.tar.gz"
rm -f -- "$archive" "$archive.sha256"
tar -C "$stage" -czf "$archive" bin share
(cd "$output_dir" && sha256sum "$(basename -- "$archive")" > "$(basename -- "$archive").sha256")
echo "Created $archive"
cat "$archive.sha256"
