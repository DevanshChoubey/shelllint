#!/usr/bin/env python3
"""Run Haskell shellcheck and the Rust port on each input; report median wall
time, peak RSS (/usr/bin/time -v) and whether the (line,col,code) sets match.
Each process checks one file, single-threaded per file."""
import json, os, re, statistics, subprocess, sys, time, glob

here = os.path.dirname(os.path.abspath(__file__))
HS = os.path.join(here, ".bin", "shellcheck-hs")
RS = os.environ["RUST_BIN"]
RUNS = int(os.environ.get("RUNS", "7"))
TIMEOUT = int(os.environ.get("TIMEOUT", "120"))
ARGS = ["-f", "json1", "--norc", "-o", "all"]

def run(cmd):
    t0 = time.perf_counter()
    try:
        p = subprocess.run(["/usr/bin/time", "-v"] + cmd, capture_output=True, timeout=TIMEOUT)
    except subprocess.TimeoutExpired:
        return None, None, b"", "timeout"
    wall = time.perf_counter() - t0
    err = p.stderr.decode(errors="replace")
    m = re.search(r"Maximum resident set size \(kbytes\): (\d+)", err)
    rss = int(m.group(1)) if m else None
    status = "ok" if p.returncode in (0, 1) else f"exit {p.returncode}"
    return wall, rss, p.stdout, status

def codes(out):
    try:
        return sorted((c["line"], c["column"], c["code"]) for c in json.loads(out)["comments"])
    except Exception:
        return None

def measure(cmd):
    walls, rsss, out, status = [], [], b"", "ok"
    for i in range(RUNS):
        w, r, o, s = run(cmd)
        if w is None or s != "ok":
            return None, None, o, s
        walls.append(w); rsss.append(r); out = o
    return statistics.median(walls), max(rsss), out, status

inputs = sorted(glob.glob(os.path.join(here, "handwritten", "*.sh")) + glob.glob(os.path.join(here, "inputs", "*.sh")))
rows = []
for path in inputs:
    name = os.path.basename(path)
    hw, hr, ho, hs_ = measure([HS] + ARGS + [path])
    rw, rr, ro, rs_ = measure([RS] + ARGS + [path])
    hc, rc = codes(ho), codes(ro)
    if hc is None or rc is None:
        match = "n/a"
        extra = ""
    else:
        hset, rset = {c[2] for c in hc}, {c[2] for c in rc}
        match = "exact" if hc == rc else ("codes-equal" if hset == rset else "DIFF")
        extra = f"hs={len(hc)} rs={len(rc)} onlyHS={sorted(hset-rset)} onlyRS={sorted(rset-hset)}"
    rows.append(dict(name=name, lines=sum(1 for _ in open(path, "rb")), hs_wall=hw, hs_rss=hr, hs_status=hs_,
                     rs_wall=rw, rs_rss=rr, rs_status=rs_, match=match, extra=extra))
    print(json.dumps(rows[-1]), file=sys.stderr, flush=True)

def f(x, fmt): return "-" if x is None else fmt % x
print("| input | lines | Haskell time (s) | Rust time (s) | speedup | Haskell RSS (MB) | Rust RSS (MB) | RSS ratio | diagnostics (line,col,code) |")
print("|---|---:|---:|---:|---:|---:|---:|---:|---|")
for r in rows:
    sp = f(r["hs_wall"] / r["rs_wall"], "%.1fx") if r["hs_wall"] and r["rs_wall"] else "-"
    rr = f(r["hs_rss"] / r["rs_rss"], "%.1fx") if r["hs_rss"] and r["rs_rss"] else "-"
    hw = f(r["hs_wall"], "%.3f") if r["hs_status"] == "ok" else r["hs_status"]
    rw = f(r["rs_wall"], "%.3f") if r["rs_status"] == "ok" else r["rs_status"]
    print(f"| {r['name']} | {r['lines']} | {hw} | {rw} | {sp} | {f(r['hs_rss'] and r['hs_rss']/1024, '%.1f')} | {f(r['rs_rss'] and r['rs_rss']/1024, '%.1f')} | {rr} | {r['match']} |")
print()
for r in rows:
    if r["extra"]:
        print(f"- {r['name']}: {r['extra']}")
