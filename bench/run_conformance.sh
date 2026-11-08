#!/usr/bin/env bash
# Corpus conformance report (JSON) for site/generate.py.
# Writes bench/results/conformance.json. Exit status is ignored on purpose:
# the report exits 1 when any corpus test fails, and the failures are data.
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/media/devansh/Experiment/playground/shellcheck-rust-targets/site}
mkdir -p bench/results
(ulimit -v 8000000; timeout 1200 flock /tmp/shellcheck-rust-cargo.lock nice \
  cargo run -q -p shelllint-conformance -j 4 -- report --json) > bench/results/conformance.json || true
python3 -I -c 'import json,sys; d=json.load(open(sys.argv[1])); print("pass %(pass)d / fail %(fail)d / blocked %(blocked)d / unported %(unported)d" % d)' \
  bench/results/conformance.json
