#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# DIVERGENCE: shelllint-only tooling for the zsh support work (no ShellCheck counterpart).
"""Run shelllint over a corpus of zsh files and report what must not happen.

    python3 -I tools/zsh_corpus.py PATH [--bin PATH_TO_SHELLLINT] [--jobs N]

The binary comes from --bin, else the SHELLLINT environment variable, else `shelllint`
on PATH. Every `*.zsh` and `*.zsh-theme` file under PATH is linted with
`--shell=zsh -f json1` (plus `--norc`, so a user rc file cannot change the result), with a
5 second timeout per file. Symbolic links (files and directories) are not followed.

Reported categories (the exit status is 1 if any is non-empty, 2 for usage errors):

  hard parse    a hard parse code was reported (SC1072 1073 1036 1058 1070 1141 1054 1056
                1055 1009 1027 1088)
  crash         exit status other than 0/1, a panic on stderr, or unreadable JSON
  timeout       more than 5 seconds
  class (d)     a class (d) code was reported: no zsh-true form exists, so zsh mode must be
                silent (SC2296 2298 2299 2300 2301 2082 2190 2051 2277 2270 2168 2180 2338)
  class (b)     a class (b) code was reported in a file that never mentions the option that
                makes it true (SC2086 2206 2048 2068 2223 2053 2295 2254 2128 1087 2178 2028)

A file "mentions the enabling option" if its text contains shwordsplit, globsubst,
ksharrays or bsdecho in any spelling (case-insensitive, underscores ignored, so
`SH_WORD_SPLIT` and `setopt noksharrays` both count), or `emulate sh` / `emulate ksh`
(with any flags). Those are the cases in which a class (b) code can legitimately fire.

Files using only the explicit forms `${=x}`, `${~x}`, `$=x`, `$~x` are still counted as
class (b) offenders (the strict rule), and are also listed separately as an informational
count, because a class (b) code on an explicit form may be a legitimate finding.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

TIMEOUT = 5
HARD = {1072, 1073, 1036, 1058, 1070, 1141, 1054, 1056, 1055, 1009, 1027, 1088}
CLASS_D = {2296, 2298, 2299, 2300, 2301, 2082, 2190, 2051, 2277, 2270, 2168, 2180, 2338}
CLASS_B = {2086, 2206, 2048, 2068, 2223, 2053, 2295, 2254, 2128, 1087, 2178, 2028}

ENABLING = ("shwordsplit", "globsubst", "ksharrays", "bsdecho")
EMULATE = re.compile(r"\bemulate\b(?:\s+-\w+)*\s+(?:sh|ksh)\b")
EXPLICIT = re.compile(r"\$\{[=~]|\$[=~]\w")


def mentions_enabling(text):
    squashed = text.lower().replace("_", "")
    if any(o in squashed for o in ENABLING):
        return True
    return bool(EMULATE.search(text))


def find_files(root):
    out = []
    if os.path.isfile(root) and not os.path.islink(root):
        return [root]
    for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
        dirnames[:] = sorted(d for d in dirnames if not os.path.islink(os.path.join(dirpath, d)))
        for f in sorted(filenames):
            p = os.path.join(dirpath, f)
            if os.path.islink(p):
                continue
            if f.endswith(".zsh") or f.endswith(".zsh-theme"):
                out.append(p)
    return out


def lint(binary, path):
    """Return (status, codes, detail). status in ok, crash, timeout."""
    try:
        p = subprocess.run(
            [binary, "--norc", "--shell=zsh", "-f", "json1", path],
            capture_output=True,
            timeout=TIMEOUT,
        )
    except subprocess.TimeoutExpired:
        return "timeout", [], "no result in %ds" % TIMEOUT
    stderr = p.stderr.decode("utf-8", "replace")
    if "panicked" in stderr or "RUST_BACKTRACE" in stderr:
        return "crash", [], "panic: " + stderr.strip().splitlines()[0][:200]
    if p.returncode not in (0, 1):
        return "crash", [], "exit %s: %s" % (p.returncode, stderr.strip()[:200])
    try:
        data = json.loads(p.stdout)
        codes = [(c["code"], c["line"]) for c in data["comments"]]
    except (ValueError, KeyError, TypeError) as e:
        return "crash", [], "unreadable json output: %s" % e
    return "ok", codes, ""


def main(argv):
    ap = argparse.ArgumentParser(description="shelllint over a zsh corpus")
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
        results = list(ex.map(lambda f: lint(binary, f), files))

    hard, crash, timeout, class_d, class_b = {}, {}, {}, {}, {}
    explicit_only = {}
    parsed_ok = 0
    for f, (status, codes, detail) in zip(files, results):
        if status == "timeout":
            timeout[f] = detail
            continue
        if status == "crash":
            crash[f] = detail
            continue
        h = [(c, l) for c, l in codes if c in HARD]
        if h:
            hard[f] = h
        else:
            parsed_ok += 1
        d = [(c, l) for c, l in codes if c in CLASS_D]
        if d:
            class_d[f] = d
        b = [(c, l) for c, l in codes if c in CLASS_B]
        if b:
            with open(f, "rb") as fh:
                text = fh.read().decode("utf-8", "replace")
            if not mentions_enabling(text):
                class_b[f] = b
                if EXPLICIT.search(text):
                    explicit_only[f] = b

    def counts(d):
        n = {}
        for lst in d.values():
            for c, _ in lst:
                n[c] = n.get(c, 0) + 1
        return ", ".join("SC%d x%d" % (c, n[c]) for c in sorted(n))

    total = len(files)
    print("shelllint zsh corpus run: %s" % args.path)
    print("binary: %s" % binary)
    print("files: %d" % total)
    print("parsed with no hard parse code: %d/%d" % (parsed_ok, total))
    print("files with hard parse codes:    %d  %s" % (len(hard), counts(hard)))
    print("crashes:                        %d" % len(crash))
    print("timeouts:                       %d" % len(timeout))
    print("files with class (d) codes:     %d  %s" % (len(class_d), counts(class_d)))
    print("files with class (b) codes but no enabling option mentioned: %d  %s" % (len(class_b), counts(class_b)))

    print("  of which use an explicit ${=}/${~} form (informational): %d" % len(explicit_only))

    def section(title, d, fmt):
        if not d:
            return
        print("\n== %s (%d)" % (title, len(d)))
        for f in sorted(d):
            print("  %s: %s" % (os.path.relpath(f, args.path) if os.path.isdir(args.path) else f, fmt(d[f])))

    pl = lambda lst: ", ".join("SC%d@%d" % (c, l) for c, l in lst[:12]) + (" ..." if len(lst) > 12 else "")
    section("hard parse codes", hard, pl)
    section("crashes", crash, str)
    section("timeouts", timeout, str)
    section("class (d) codes", class_d, pl)
    section("class (b) codes without the enabling option", class_b, pl)

    bad = bool(hard or crash or timeout or class_d or class_b)
    print("\nresult: %s" % ("FAIL" if bad else "ok"))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
