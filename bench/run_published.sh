#!/usr/bin/env bash
# Produces the numbers in docs/benchmarks.md: the real-repo and synthetic
# benchmarks inside a container limited to 6 GB of real memory, no swap.
# Published runs use a cgroup limit because `ulimit -v` limits address space,
# which the Haskell runtime reserves far beyond what it uses.
# Needs docker, the release build below, and bench/.bin/shellcheck-hs.
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$PWD/target}
LOCK=${LOCK:-/tmp/shellcheck-rust-cargo.lock}
CPUS=${CPUS:-4-7}

bench/fetch.sh
(ulimit -v 8000000; timeout 1500 flock "$LOCK" nice cargo build -q --release --locked -p shelllint -j 4)

timeout 3400 flock "$LOCK" docker run --rm --memory=6g --memory-swap=6g --cpuset-cpus="$CPUS" \
  -v "$PWD":"$PWD" -w "$PWD" -v "$CARGO_TARGET_DIR/release/shelllint":/opt/shelllint:ro \
  -e RUSTC_VERSION="$(rustc --version)" -e HOST_UID="$(id -u):$(id -g)" -e CPUS="$CPUS" \
  ubuntu:26.04 bash -c '
set -euo pipefail
apt-get -qq update >/dev/null && apt-get -qq install -y python3 time lsb-release >/dev/null 2>&1
export RS=/opt/shelllint HS=$PWD/bench/.bin/shellcheck-hs MEM_KB=0 RUNS=3
export MEM_CAP_NOTE="cgroup limit 6 GB real memory, no swap (docker --memory=6g --memory-swap=6g), no address-space limit, pinned to CPUs $CPUS"
python3 -I bench/real_bench.py
python3 -I bench/gen_inputs.py >/dev/null
RUST_BIN=/opt/shelllint RUNS=5 TIMEOUT=120 python3 -I bench/measure.py > bench/results/synthetic.md 2> bench/results/synthetic.jsonl
chown -R "$HOST_UID" bench/results bench/inputs
'
