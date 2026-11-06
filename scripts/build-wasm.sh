#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Builds crates/wasm for the browser playground into playground/pkg/.
#
# STATUS: BLOCKED. The wasm32-unknown-unknown link does not compile:
# linkme 0.3 (the checks registry, `#[linkme::distributed_slice(CHECKS)]` in
# ~220 places) has no implementation for target_os = "unknown"
# ("distributed_slice is not implemented for this platform"). Fixing it needs
# the registry replaced on all targets or on wasm only; not done yet.
#
# Runs inside docker, so the host needs no Rust toolchain or wasm32 target.
# The wasm-bindgen CLI version is read from Cargo.lock, so it always matches
# the library. Cargo's registry and build cache live in named docker volumes
# and are reused across builds.
#
# Usage: scripts/build-wasm.sh            (no wasm-opt)
#        WASM_OPT=1 scripts/build-wasm.sh (also runs binaryen's wasm-opt -Oz)
# Env:   RUST_IMAGE (default rust:1.93-bookworm; must be >= the workspace
#        MSRV, edition 2024 needs 1.85).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUST_IMAGE="${RUST_IMAGE:-rust:1.93-bookworm}"
WASM_OPT="${WASM_OPT:-0}"

BINDGEN_VERSION="$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/"/,"",$3); print $3; exit}' "$ROOT/Cargo.lock")"
if [[ -z "$BINDGEN_VERSION" ]]; then
  echo "could not find wasm-bindgen in Cargo.lock" >&2
  exit 1
fi
echo "rust image: $RUST_IMAGE, wasm-bindgen: $BINDGEN_VERSION, wasm-opt: $WASM_OPT"

docker run --rm --memory=6g --cpus=4 \
  -e BINDGEN_VERSION="$BINDGEN_VERSION" \
  -e WASM_OPT="$WASM_OPT" \
  -e CARGO_HOME=/cargo-home \
  -e CARGO_TARGET_DIR=/cargo-target \
  -v "$ROOT":/src \
  -v shellcheck-wasm-cargo-home:/cargo-home \
  -v shellcheck-wasm-cargo-target:/cargo-target \
  -w /src \
  "$RUST_IMAGE" sh -eu -c '
    rustup target add wasm32-unknown-unknown
    if ! wasm-bindgen --version 2>/dev/null | grep -q "$BINDGEN_VERSION"; then
      cargo install wasm-bindgen-cli --version "=$BINDGEN_VERSION" --locked -j 4
    fi
    cargo build -p shellcheck-wasm --release --target wasm32-unknown-unknown -j 4 --locked
    OUT=/src/playground/pkg
    rm -rf "$OUT" && mkdir -p "$OUT"
    wasm-bindgen --target web --no-typescript --out-dir "$OUT" --out-name shellcheck_wasm \
      /cargo-target/wasm32-unknown-unknown/release/shellcheck_wasm.wasm
    if [ "$WASM_OPT" = 1 ]; then
      apt-get update -qq && apt-get install -y -qq binaryen >/dev/null
      wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
        "$OUT/shellcheck_wasm_bg.wasm" -o "$OUT/shellcheck_wasm_bg.wasm"
    fi
    # Files are written as root inside the container; hand them back to the caller.
    chown -R '"$(id -u):$(id -g)"' "$OUT"
  '
ls -l "$ROOT/playground/pkg"
