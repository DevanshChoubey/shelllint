#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# DIVERGENCE: shelllint-only tooling for the zsh support work (no ShellCheck counterpart).
"""No-panic sweep: truncate every corpus file at every 64th byte offset and lint it.

    python3 -I tools/zsh_truncation_sweep.py PATH [--bin PATH_TO_SHELLLINT] [--jobs N]

For every `*.zsh` / `*.zsh-theme` file under PATH (symbolic links not followed) and every
offset 64, 128, ... below the file size, the first `offset` bytes are fed to
`shelllint --shell=zsh -f json1 -` on stdin with a 2 second timeout. A run is flagged if it
panics (panic text on stderr), crashes (exit status other than 0/1, including death by
signal) or times out. Parse errors in the truncated text are expected and fine.

This is a local tool (a full oh-my-zsh sweep is tens of thousands of runs), not for CI.
The exit status is 1 if anything was flagged, 2 for usage errors. The first flagged
offsets per file are printed so the input can be rebuilt with `head -c OFFSET FILE`.
"""
import argparse
import os
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

STEP = 64
TIMEOUT = 2


def find_files(root):
    out = []
    if os.path.isfile(root) and not os.path.islink(root):
        return [root]
    for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
        dirnames[:] = sorted(d for d in dirnames if not os.path.islink(os.path.join(dirpath, d)))
        for f in sorted(filenames):
            p = os.path.join(dirpath, f)
            if not os.path.islink(p) and (f.endswith(".zsh") or f.endswith(".zsh-theme")):
                out.append(p)
    return out


def run_one(binary, data):
    try:
        p = subprocess.run(
            [binary, "--norc", "--shell=zsh", "-f", "json1", "-"],
            input=data,
            capture_output=True,
            timeout=TIMEOUT,
        )
    except subprocess.TimeoutExpired:
        return "timeout"
    err = p.stderr.decode("utf-8", "replace")
    if "panicked" in err or "RUST_BACKTRACE" in err:
        return "panic"
    if p.returncode not in (0, 1):
        return "crash(exit %s)" % p.returncode
    return None


def sweep_file(binary, path):
    with open(path, "rb") as fh:
        data = fh.read()
    flagged, runs = [], 0
    for off in range(STEP, len(data), STEP):
        runs += 1
        r = run_one(binary, data[:off])
        if r:
            flagged.append((off, r))
    return path, runs, flagged


def main(argv):
    ap = argparse.ArgumentParser(description="truncation no-panic sweep")
    ap.add_argument("path")
    ap.add_argument("--bin", default=None, help="shelllint binary (default: $SHELLLINT, else shelllint on PATH)")
    ap.add_argument("--jobs", type=int, default=4)
    args = ap.parse_args(argv)
    binary = args.bin or os.environ.get("SHELLLINT") or shutil.which("shelllint")
    if not binary or not os.path.exists(binary):
        print("error: no shelllint binary (use --bin or SHELLLINT)", file=sys.stderr)
        return 2
    if not os.path.exists(args.path):
        print("error: no such path: %s" % args.path, file=sys.stderr)
        return 2
    files = find_files(args.path)
    if not files:
        print("error: no *.zsh / *.zsh-theme files under %s" % args.path, file=sys.stderr)
        return 2
    with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as ex:
        results = list(ex.map(lambda f: sweep_file(binary, f), files))
    runs = sum(r for _, r, _ in results)
    bad = [(p, fl) for p, _, fl in results if fl]
    kinds = {}
    for _, fl in bad:
        for _, k in fl:
            kinds[k] = kinds.get(k, 0) + 1
    print("truncation sweep: %s" % args.path)
    print("binary: %s" % binary)
    print("files: %d, runs: %d (every %d bytes, %ds timeout)" % (len(files), runs, STEP, TIMEOUT))
    print("flagged runs: %d in %d files  %s" % (sum(kinds.values()), len(bad), dict(sorted(kinds.items()))))
    for p, fl in bad:
        shown = ", ".join("%d:%s" % (o, k) for o, k in fl[:8]) + (" ..." if len(fl) > 8 else "")
        print("  %s: %s" % (os.path.relpath(p, args.path) if os.path.isdir(args.path) else p, shown))
    print("\nresult: %s" % ("FAIL" if bad else "ok"))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
