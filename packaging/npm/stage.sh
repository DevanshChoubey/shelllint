#!/bin/sh
# Copy a built binary into the platform package it belongs to, e.g.
#   packaging/npm/scripts-stage.sh target/release/shelllint linux-x64
# Run from the repo root before `npm pack` in the platform directory.
set -eu
src="$1"; key="$2"
dir="$(cd "$(dirname "$0")" && pwd)/platforms/$key/bin"
case "$key" in win32-*) out="$dir/shelllint.exe" ;; *) out="$dir/shelllint" ;; esac
cp "$src" "$out"
chmod +x "$out"
echo "staged $src -> $out"
