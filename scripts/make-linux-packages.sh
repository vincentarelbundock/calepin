#!/usr/bin/env bash
# Build the .deb, the .rpm and the Arch .pkg.tar.zst from one description
# (packaging/linux/nfpm.yaml).
#
# Usage:
#   ./scripts/make-linux-packages.sh [binary] [arch]
#
# `binary` defaults to target/release/calepin, `arch` to the machine's own.
# Both are arguments because the release workflow packages the binaries
# cargo-dist already built and published, for two architectures, rather than
# rebuilding them here: the bytes in the .deb are then the same bytes users
# get from the tarball and the shell installer.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

version="$(awk -F'"' '/^version/ { print $2; exit }' calepin/Cargo.toml)"
target="${1:-$root/target/release/calepin}"
staging="$root/dist/linux-staging"

if [ -n "${2:-}" ]; then
  arch="$2"
else
  case "$(uname -m)" in
    x86_64)  arch=amd64 ;;
    aarch64) arch=arm64 ;;
    *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
  esac
fi

case "$arch" in
  amd64|arm64) ;;
  *) echo "unsupported arch argument: $arch (expected amd64 or arm64)" >&2; exit 1 ;;
esac

if ! command -v nfpm >/dev/null 2>&1; then
  cat >&2 <<'MISSING'
nfpm is not installed.

All three packages are generated from one description rather than maintained
three times, and nfpm is what does that. Install a release from
https://github.com/goreleaser/nfpm/releases and put it on PATH.
MISSING
  exit 1
fi

if [ ! -f "$target" ]; then
  echo "no binary at $target — run 'make build-release' first" >&2
  exit 1
fi

# A binary built on NixOS names its ELF interpreter in /nix/store, so the
# package installs cleanly on Debian and then fails with "cannot execute:
# required file not found". The release workflow is unaffected because it
# packages cargo-dist's binaries, but a local `make linux-packages` on NixOS
# would otherwise produce a package that looks fine and does not run.
if command -v readelf >/dev/null 2>&1 &&
   readelf -p .interp "$target" 2>/dev/null | grep -q /nix/store; then
  echo "refusing to package: $target has a Nix ELF interpreter" >&2
  echo "unpack a cargo-dist release tarball and package that binary instead:" >&2
  echo "  ./scripts/make-linux-packages.sh /path/to/unpacked/calepin amd64" >&2
  exit 1
fi

rm -rf "$staging"
mkdir -p "$staging" "$root/dist"
install -m755 "$target" "$staging/calepin"

export CALEPIN_VERSION="$version" CALEPIN_ARCH="$arch"
for packager in deb rpm archlinux; do
  nfpm package --packager "$packager" \
    --config packaging/linux/nfpm.yaml --target dist/
done
# A binary built on NixOS names its ELF interpreter in /nix/store, so the
# package installs cleanly on Debian and then fails with "cannot execute:
# required file not found". The release workflow is unaffected because it
# packages cargo-dist's binaries, but a local `make linux-packages` on NixOS
# would otherwise produce a package that looks fine and does not run.
if command -v readelf >/dev/null 2>&1; then
  interp="$(readelf -p .interp "$target" 2>/dev/null | grep -o '"'"'/[^ ]*ld-linux[^ ]*'"'"' || true)"
  case "$interp" in
    /nix/store/*)
      echo "refusing to package: $target has a Nix ELF interpreter ($interp)" >&2
      echo "pass a binary from a cargo-dist release tarball instead:" >&2
      echo "  ./scripts/make-linux-packages.sh /path/to/unpacked/calepin amd64" >&2
      exit 1
      ;;
  esac
fi

rm -rf "$staging"

for artifact in dist/calepin*.deb dist/calepin*.rpm dist/calepin*.pkg.tar.zst; do
  [ -e "$artifact" ] || continue
  echo "package: $artifact"
done
