#!/bin/sh
# Copy a built binary into the platform package it belongs to, e.g.
#   packaging/npm/scripts-stage.sh target/release/shellcheck-rust linux-x64
# Run from the repo root before `npm pack` in the platform directory.
set -eu
src="$1"; key="$2"
dir="$(cd "$(dirname "$0")" && pwd)/platforms/$key/bin"
case "$key" in win32-*) out="$dir/shellcheck-rust.exe" ;; *) out="$dir/shellcheck-rust" ;; esac
cp "$src" "$out"
chmod +x "$out"
echo "staged $src -> $out"
