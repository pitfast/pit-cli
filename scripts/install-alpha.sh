#!/usr/bin/env bash
set -euo pipefail

version=""
prefix="${HOME:?HOME must be set}/.local"
archive=""
checksum=""
url=""
checksum_url=""
uninstall=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) version="${2:?--version requires a value}"; shift 2 ;;
    --prefix) prefix="${2:?--prefix requires a directory}"; shift 2 ;;
    --archive) archive="${2:?--archive requires a file}"; shift 2 ;;
    --checksum) checksum="${2:?--checksum requires a file}"; shift 2 ;;
    --url) url="${2:?--url requires a URL}"; shift 2 ;;
    --checksum-url) checksum_url="${2:?--checksum-url requires a URL}"; shift 2 ;;
    --uninstall) uninstall=1; shift ;;
    --help|-h)
      echo "Usage: $0 [--archive FILE --checksum FILE] [--url URL --checksum-url URL] [--version VERSION] [--prefix DIR]"
      echo "       $0 --uninstall [--prefix DIR]"
      exit 0
      ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

base="$prefix/share/pitfast"
if [[ "$uninstall" -eq 1 ]]; then
  rm -f -- "$prefix/bin/pit"
  rm -rf -- "$base/versions"
  rm -f -- "$base/current"
  echo "Removed PitFast-managed files under $prefix (project data and caches were preserved)."
  exit 0
fi

download_dir=""
if [[ -n "$url" ]]; then
  command -v curl >/dev/null || { echo "curl is required for URL installation" >&2; exit 1; }
  download_dir="$(mktemp -d "${TMPDIR:-/tmp}/pitfast-download.XXXXXX")"
  archive="$download_dir/bundle.tar.gz"
  checksum="$download_dir/bundle.tar.gz.sha256"
  curl --fail --location --silent --show-error --output "$archive" "$url"
  checksum_url="${checksum_url:-${url}.sha256}"
  curl --fail --location --silent --show-error --output "$checksum" "$checksum_url"
fi
[[ -n "$archive" ]] || { echo "provide --archive or --url" >&2; exit 2; }
[[ -f "$archive" ]] || { echo "archive not found: $archive" >&2; exit 1; }
if [[ -z "$checksum" && -f "$archive.sha256" ]]; then checksum="$archive.sha256"; fi
[[ -n "$checksum" && -f "$checksum" ]] || { echo "a SHA-256 checksum file is required" >&2; exit 1; }

archive="$(cd -- "$(dirname -- "$archive")" && pwd)/$(basename -- "$archive")"
checksum="$(cd -- "$(dirname -- "$checksum")" && pwd)/$(basename -- "$checksum")"
expected="$(awk 'NF {print $1; exit}' "$checksum")"
actual="$(sha256sum "$archive" | awk '{print $1}')"
[[ "$expected" == "$actual" ]] || {
  echo "checksum verification failed for $archive" >&2
  exit 1
}

extract="$(mktemp -d "${TMPDIR:-/tmp}/pitfast-install.XXXXXX")"
trap 'rm -rf -- "$extract" "$download_dir"' EXIT
tar -xzf "$archive" -C "$extract"
manifest="$extract/share/release-manifest.json"
[[ -f "$manifest" ]] || { echo "bundle is missing share/release-manifest.json" >&2; exit 1; }
readarray -t metadata < <(python3 - "$manifest" "$version" <<'PY'
import json
import platform
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    manifest = json.load(stream)
requested = sys.argv[2]
if manifest.get("schema") != 1:
    raise SystemExit("unsupported PitFast release manifest schema")
if requested and manifest.get("version") != requested:
    raise SystemExit(f"bundle version {manifest.get('version')} does not match requested {requested}")
if (
    manifest.get("os") != "linux"
    or manifest.get("architecture") != "x86_64"
    or platform.machine() not in ("x86_64", "amd64")
):
    raise SystemExit("this alpha bundle supports Linux x86_64 only")
print(manifest["version"])
print(manifest["target"])
PY
)
installed_version="${metadata[0]}"
target="${metadata[1]}"
for binary in pit pit-lane; do
  [[ -x "$extract/bin/$binary" ]] || { echo "bundle is missing executable $binary" >&2; exit 1; }
done

version_dir="$base/versions/$installed_version"
mkdir -p -- "$base/versions" "$prefix/bin"
temporary="$base/versions/.${installed_version}.tmp.$$"
rm -rf -- "$temporary"
mkdir -- "$temporary"
cp -a -- "$extract/bin" "$extract/share" "$temporary/"
chmod -R a+rX,u+w -- "$temporary"
if [[ -e "$version_dir" ]]; then
  [[ -x "$version_dir/bin/pit" && -f "$version_dir/share/release-manifest.json" ]] || {
    echo "managed version directory is incomplete; refusing to overwrite $version_dir" >&2
    exit 1
  }
  rm -rf -- "$temporary"
else
  mv -- "$temporary" "$version_dir"
fi
current_tmp="$base/.current.$$"
ln -sfn -- "$version_dir" "$current_tmp"
mv -Tf -- "$current_tmp" "$base/current"
ln -sfn -- "$base/current/bin/pit" "$prefix/bin/pit"
"$prefix/bin/pit" --version
case ":${PATH}:" in
  *":$prefix/bin:"*) ;;
  *) echo "Add $prefix/bin to PATH, for example: export PATH=\"$prefix/bin:\$PATH\"" ;;
esac
echo "Installed PitFast $installed_version ($target) under $version_dir"
