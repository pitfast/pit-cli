#!/usr/bin/env bash
set -euo pipefail

prefix="${HOME:?HOME must be set}/.local/bin"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      [[ $# -ge 2 ]] || { echo "--prefix requires a directory" >&2; exit 2; }
      prefix="$2"
      shift 2
      ;;
    --help|-h)
      echo "Usage: $0 [--prefix DIRECTORY]"
      echo "Build and install the local PitFast CLI without requiring root."
      echo "For a versioned CLI + local runtime bundle, use scripts/install-alpha.sh."
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 2
      ;;
  esac
done

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd -- "$script_dir/.." && pwd)"
mkdir -p -- "$prefix"

cargo build --manifest-path "$repo_dir/Cargo.toml" --release
binary="$repo_dir/target/release/pit"
[[ -x "$binary" ]] || { echo "release build did not produce $binary" >&2; exit 1; }

destination="$prefix/pit"
temporary="$prefix/.pit.tmp.$$"
cp -- "$binary" "$temporary"
mv -- "$temporary" "$destination"
"$destination" --version

case ":${PATH}:" in
  *":$prefix:"*) ;;
  *) echo "Add $prefix to PATH, for example: export PATH=\"$prefix:\$PATH\"" ;;
esac
echo "Installed PitFast CLI at $destination"
