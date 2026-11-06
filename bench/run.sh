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

# 2. Rust side. The CLI crate is still a stub, so use the checker example
#    (crates/checker/examples/sc_bench.rs); it mirrors
#    `shellcheck -f json1 --norc -o all FILE`. Switch to the CLI binary here
#    once it is wired.
cargo build --release -p shellcheck-checker --example sc_bench
export RUST_BIN=$CARGO_TARGET_DIR/release/examples/sc_bench

# 3. Inputs.
python3 bench/gen_inputs.py

# 4. Measure.
python3 bench/measure.py

# 5. Optional parallel-files mode (separate from the single-thread table).
if [ -n "${PARALLEL:-}" ]; then
  echo; echo "parallel-files mode (Rust: one thread per file; Haskell: xargs -P):"
  files=(bench/handwritten/*.sh bench/inputs/gen_*.sh bench/inputs/corpus_concat.sh)
  /usr/bin/time -f "rust --parallel: %e s, %M KB" "$RUST_BIN" --quiet --parallel "${files[@]}"
  /usr/bin/time -f "haskell xargs -P$(nproc): %e s" sh -c \
    "printf '%s\n' ${files[*]} | xargs -P$(nproc) -n1 bench/.bin/shellcheck-hs -f json1 --norc -o all >/dev/null || true"
fi

if [ -n "${CRITERION:-}" ]; then
  cargo bench -p shellcheck-checker
fi
