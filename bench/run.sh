#!/usr/bin/env bash
# Benchmark harness: Rust port vs Haskell ShellCheck (wall time + peak RSS).
#   bench/run.sh            full run (RUNS=7 per input and side)
#   RUNS=3 bench/run.sh     quicker
#   CRITERION=1 bench/run.sh also run the criterion micro-benchmarks
# Output: markdown table on stdout, per-input JSON lines on stderr.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$PWD/target}

# 1. Haskell reference: static binary out of the docker image (no docker
#    startup in the measurements).
if [ ! -x bench/.bin/shellcheck-hs ]; then
  mkdir -p bench/.bin
  cid=$(docker create koalaman/shellcheck:latest)
  docker cp "$cid:/bin/shellcheck" bench/.bin/shellcheck-hs
  docker rm "$cid" >/dev/null
fi

# 2. Rust side: the real CLI.
cargo build --release -p shelllint
export RUST_BIN=$CARGO_TARGET_DIR/release/shelllint

# 3. Inputs.
python3 bench/gen_inputs.py

# 4. Measure.
python3 bench/measure.py

# 5. Optional parallel-files mode (separate from the single-thread table):
#    N files through xargs -P on both sides (process-level parallelism).
if [ -n "${PARALLEL:-}" ]; then
  echo; echo "parallel-files mode (xargs -P$(nproc), 4 copies of each mid-size input):"
  files=$(for i in 1 2 3 4; do ls bench/handwritten/*.sh bench/inputs/gen_1k.sh bench/inputs/gen_10k.sh bench/inputs/corpus_concat.sh; done)
  for bin in "$RUST_BIN" bench/.bin/shellcheck-hs; do
    echo "$bin"
    # shellcheck disable=SC2086
    printf '%s\n' $files | /usr/bin/time -f "  %e s wall" xargs -P"$(nproc)" -n1 "$bin" -f json1 --norc -o all >/dev/null || true
  done
fi

if [ -n "${CRITERION:-}" ]; then
  cargo bench -p shelllint-checker
fi
