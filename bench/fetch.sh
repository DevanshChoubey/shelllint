#!/usr/bin/env bash
# Shallow-clone each repo in bench/repos.toml at its pinned commit into
# bench/real/<name> (git-ignored). Idempotent: a clone already at the pinned
# commit is left alone.
#   bench/fetch.sh            fetch everything
#   REAL_DIR=/path bench/fetch.sh
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail
cd "$(dirname "$0")/.."
DEST=${REAL_DIR:-$PWD/bench/real}
mkdir -p "$DEST"

python3 -I -c '
import sys, tomllib
with open("bench/repos.toml", "rb") as f:
    for r in tomllib.load(f)["repo"]:
        c = r["commit"]
        if len(c) != 40 or any(ch not in "0123456789abcdef" for ch in c):
            sys.exit("%s: commit must be a full lowercase 40-hex SHA" % r["name"])
        print(r["name"], r["url"], r["commit"], sep="\t")
' | while IFS=$'\t' read -r name url commit; do
  dir="$DEST/$name"
  if [ -d "$dir/.git" ] && [ "$(git -C "$dir" rev-parse HEAD 2>/dev/null || true)" = "$commit" ]; then
    echo "ok       $name @ ${commit:0:7}"
    continue
  fi
  rm -rf "$dir"
  mkdir -p "$dir"
  git -C "$dir" init -q
  git -C "$dir" remote add origin "$url"
  # GitHub allows fetching an arbitrary reachable SHA with --depth 1.
  git -C "$dir" fetch -q --depth 1 origin "$commit"
  git -C "$dir" -c advice.detachedHead=false checkout -q FETCH_HEAD
  echo "fetched  $name @ ${commit:0:7}"
done
