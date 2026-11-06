#!/usr/bin/env bash
# Real-repo benchmark: Haskell ShellCheck vs the Rust release binary over the
# shell files of the pinned repos in bench/repos.toml.
#
#   bench/run_real.sh          build, fetch, measure, write bench/results/
#   RUNS=5 bench/run_real.sh   more timing repetitions (default 3)
#
# Outputs:
#   bench/results/real-repos.md    human-readable tables and mismatch list
#   bench/results/real-repos.json  full results (input to site/generate.py)
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/media/devansh/Experiment/playground/shellcheck-rust-targets/site}

# 1. Haskell reference: static binary out of the docker image.
if [ ! -x bench/.bin/shellcheck-hs ]; then
  mkdir -p bench/.bin
  cid=$(docker create koalaman/shellcheck:latest)
  docker cp "$cid:/bin/shellcheck" bench/.bin/shellcheck-hs
  docker rm "$cid" >/dev/null
fi

# 2. Rust release binary. Cargo runs under the shared lock and memory cap
#    (feature-common rules).
(ulimit -v 8000000; timeout 1200 flock /tmp/shellcheck-rust-cargo.lock nice cargo build --release -p shellcheck-rust -j 4)
export RS=$CARGO_TARGET_DIR/release/shellcheck-rust

# 3. Pinned repos.
bench/fetch.sh

# 4. Measure and compare. Each child process runs under RLIMIT_AS = 6 GB and
#    a 300 s timeout (see real_bench.py).
export HS=$PWD/bench/.bin/shellcheck-hs
python3 -I bench/real_bench.py
