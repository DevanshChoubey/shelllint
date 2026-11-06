#!/usr/bin/env python3
"""Generate benchmark inputs into bench/inputs/ (git-ignored except handwritten/).

Uses the Haskell reference binary to keep only corpus snippets that parse
without any 1xxx (syntax) diagnostics, so concatenations stay parseable.
"""
import json, os, random, subprocess, sys

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(here)
hs = os.path.join(here, ".bin", "shellcheck-hs")
out = os.path.join(here, "inputs")
os.makedirs(out, exist_ok=True)

snips = []
seen = set()
with open(os.path.join(root, "corpus", "tests.jsonl")) as f:
    for line in f:
        d = json.loads(line)
        s = d.get("script")
        if not s or s in seen or "\x00" in s:
            continue
        seen.add(s)
        snips.append(s if s.endswith("\n") else s + "\n")

clean = []
for s in snips:
    p = subprocess.run([hs, "-f", "json1", "--norc", "-o", "all", "-"], input=s.encode(), capture_output=True)
    try:
        codes = [c["code"] for c in json.loads(p.stdout)["comments"]]
    except Exception:
        continue
    if any(1000 <= c < 2000 for c in codes):
        continue
    clean.append(s)
print(f"{len(snips)} unique corpus scripts, {len(clean)} parse cleanly", file=sys.stderr)

def write(name, text):
    with open(os.path.join(out, name), "w") as f:
        f.write(text)

def hs_codes(text):
    p = subprocess.run([hs, "-f", "json1", "--norc", "-o", "all", "-"], input=text.encode(), capture_output=True)
    return json.loads(p.stdout)["comments"]

def build(pool, picks):
    """Concatenate pool[picks]; drop any snippet that triggers a 1xxx (syntax)
    diagnostic in context (e.g. shebang/shell-directive interplay) and retry."""
    picks = list(picks)
    while True:
        starts, n = [], 2
        for i in picks:
            starts.append(n)
            n += pool[i].count("\n")
        text = "#!/bin/bash\n" + "".join(pool[i] for i in picks)
        bad = [c["line"] for c in hs_codes(text) if 1000 <= c["code"] < 2000]
        if not bad:
            return text, picks
        import bisect
        drop = {bisect.bisect_right(starts, ln) - 1 for ln in bad}
        # a syntax error can be reported at the *next* snippet; drop the one before too
        drop |= {d - 1 for d in drop if d > 0}
        picks = [p for k, p in enumerate(picks) if k not in drop]

text, kept = build(clean, range(len(clean)))
clean = [clean[i] for i in kept]
write("corpus_concat.sh", text)

def gen(nlines, seed):
    rnd = random.Random(seed)
    picks, n = [], 1
    while n < nlines:
        i = rnd.randrange(len(clean))
        k = clean[i].count("\n")
        if n + k > nlines + 40:
            continue
        picks.append(i)
        n += k
    return build(clean, picks)[0]

for n, label in [(1000, "1k"), (10000, "10k"), (50000, "50k")]:
    write(f"gen_{label}.sh", gen(n, n))

# pathological inputs (crates/conformance/tests/pathological.rs)
write("patho_nested_5000.sh", "$(" * 5000 + ")" * 5000)
write("patho_50k_lines.sh", "echo $x\n" * 50000)
write("patho_1mb_line.sh", "a" * (1024 * 1024))
