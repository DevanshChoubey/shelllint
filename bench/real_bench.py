#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Real-repo benchmark driver (called by bench/run_real.sh).

For each repo in bench/repos.toml, every sh/bash/dash/ksh file is checked by
the Haskell binary and the Rust binary, one process per file, sequentially.

Per side and repo:
  - wall time: sum of per-file process wall times, median over RUNS passes
  - peak RSS: max over all per-file processes (from /usr/bin/time -v)
  - diagnostics (run 1 output): compared on sorted (line, column, code)

Each child process runs with RLIMIT_AS = MEM_KB (same as `ulimit -v`) and a
TIMEOUT-second timeout. Hyperfine is not used: per-process peak RSS needs
/usr/bin/time, so a plain timing loop does both measurements.

Env: HS, RS (binary paths), REAL_DIR, RESULTS_DIR, RUNS, TIMEOUT, MEM_KB.
"""
import collections, datetime, json, os, platform, re, resource, shutil, statistics
import subprocess, sys, time, tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
HS = os.environ.get("HS", os.path.join(HERE, ".bin", "shellcheck-hs"))
RS = os.environ["RS"]
REAL = os.environ.get("REAL_DIR", os.path.join(HERE, "real"))
RESULTS = os.environ.get("RESULTS_DIR", os.path.join(HERE, "results"))
RUNS = int(os.environ.get("RUNS", "3"))
TIMEOUT = int(os.environ.get("TIMEOUT", "300"))
MEM_KB = int(os.environ.get("MEM_KB", "6000000"))  # same units as `ulimit -v`
ARGS = ["-f", "json1", "--norc", "-o", "all"]
SHELL_RE = re.compile(rb"\b(sh|bash|dash|ksh)\b")
RSS_RE = re.compile(r"Maximum resident set size \(kbytes\): (\d+)")


def cap_memory():
    lim = MEM_KB * 1024
    resource.setrlimit(resource.RLIMIT_AS, (lim, lim))


def sh(cmd):
    try:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=20).stdout.strip()
    except Exception as e:  # tool missing or hung
        return f"unavailable ({e.__class__.__name__})"


def machine_info():
    cpu = "unknown"
    try:
        with open("/proc/cpuinfo") as f:
            cpu = next(l.split(":", 1)[1].strip() for l in f if l.startswith("model name"))
    except (OSError, StopIteration):
        pass
    mem_gb = None
    try:
        with open("/proc/meminfo") as f:
            kb = int(next(l.split()[1] for l in f if l.startswith("MemTotal")))
            mem_gb = round(kb / 1024 / 1024, 1)
    except (OSError, StopIteration):
        pass
    return {
        "date_utc": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "cpu": cpu,
        "logical_cpus": os.cpu_count(),
        "mem_total_gb": mem_gb,
        "kernel": platform.release(),
        "os": sh(["lsb_release", "-ds"]) or platform.platform(),
        "python": platform.python_version(),
        "rustc": sh(["rustc", "--version"]),
        "loadavg_start": os.getloadavg()[0],
        "memory_cap": f"RLIMIT_AS {MEM_KB} KiB per process, timeout {TIMEOUT}s",
        "method": f"sequential, one process per file, {RUNS} passes, median of pass totals",
    }


def discover(repo_dir, excludes):
    files = []
    for dp, dn, fn in os.walk(repo_dir):
        dn[:] = [d for d in dn if d != ".git"]
        for name in fn:
            full = os.path.join(dp, name)
            rel = os.path.relpath(full, repo_dir)
            if any(rel.startswith(e) for e in excludes) or os.path.islink(full):
                continue
            if name.endswith((".sh", ".bash")):
                files.append(rel)
                continue
            try:
                with open(full, "rb") as h:
                    first = h.readline(256)
            except OSError:
                continue
            if first.startswith(b"#!") and SHELL_RE.search(first):
                files.append(rel)
    return sorted(files)


def run_one(binary, rel, cwd):
    cmd = ["/usr/bin/time", "-v", binary, *ARGS, rel]
    t0 = time.perf_counter()
    try:
        p = subprocess.run(cmd, cwd=cwd, capture_output=True, timeout=TIMEOUT, preexec_fn=cap_memory)
    except subprocess.TimeoutExpired:
        return {"status": "timeout", "wall": None, "rss": None, "out": None}
    wall = time.perf_counter() - t0
    m = RSS_RE.search(p.stderr.decode(errors="replace"))
    status = "ok" if p.returncode in (0, 1) else f"exit {p.returncode}"
    return {"status": status, "wall": wall, "rss": int(m.group(1)) if m else None,
            "out": p.stdout}


def diag_key(out):
    """Sorted (line, column, code) list from json1 output, or None if unparseable."""
    if out is None:
        return None
    try:
        comments = json.loads(out)["comments"]
        return sorted((c["line"], c["column"], c["code"]) for c in comments)
    except (ValueError, KeyError, TypeError):
        return None


def fmt_key(k):
    return f"{k[0]}:{k[1]} SC{k[2]}"


def measure(binary, files, cwd):
    """Return per-file dict and the per-pass totals for one binary."""
    walls = {f: [] for f in files}
    rss = {f: None for f in files}
    status = {f: "ok" for f in files}
    outs = {}
    pass_totals = []
    for run in range(RUNS):
        total = 0.0
        for f in files:
            r = run_one(binary, f, cwd)
            if run == 0:
                outs[f] = r["out"]
            if r["status"] != "ok":
                status[f] = r["status"]
            if r["wall"] is not None:
                walls[f].append(r["wall"])
                total += r["wall"]
            if r["rss"] is not None:
                rss[f] = max(rss[f] or 0, r["rss"])
        pass_totals.append(total)
    return {"walls": walls, "rss": rss, "status": status, "outs": outs,
            "wall_total": statistics.median(pass_totals)}


def median_or_none(xs):
    return statistics.median(xs) if xs else None


def main():
    os.makedirs(RESULTS, exist_ok=True)
    with open(os.path.join(ROOT, "bench", "repos.toml"), "rb") as f:
        repos = tomllib.load(f)["repo"]

    machine = machine_info()
    machine["hs_version"] = sh([HS, "--version"]).splitlines()[1:2]
    machine["rs_version"] = sh([RS, "--version"])
    machine["hs_binary"] = HS
    machine["rs_binary"] = RS
    machine["rs_binary_bytes"] = os.path.getsize(RS)

    results = []
    code_only_hs, code_only_rs = collections.Counter(), collections.Counter()
    for repo in repos:
        name = repo["name"]
        cwd = os.path.join(REAL, name)
        if not os.path.isdir(cwd):
            sys.exit(f"{name}: not fetched; run bench/fetch.sh first")
        files = discover(cwd, repo.get("exclude", []))
        lines = {}
        for f in files:
            with open(os.path.join(cwd, f), "rb") as h:
                lines[f] = sum(1 for _ in h)
        print(f"[{name}] {len(files)} files, {sum(lines.values())} lines", file=sys.stderr, flush=True)

        hs = measure(HS, files, cwd)
        rs = measure(RS, files, cwd)

        per_file, mismatches = [], []
        identical = compared = errors = 0
        for f in files:
            hk, rk = diag_key(hs["outs"][f]), diag_key(rs["outs"][f])
            entry = {
                "file": f, "lines": lines[f],
                "hs_wall": median_or_none(hs["walls"][f]), "rs_wall": median_or_none(rs["walls"][f]),
                "hs_rss_kb": hs["rss"][f], "rs_rss_kb": rs["rss"][f],
                "hs_status": hs["status"][f], "rs_status": rs["status"][f],
                "hs_count": None if hk is None else len(hk),
                "rs_count": None if rk is None else len(rk),
            }
            if hk is None or rk is None:
                entry["identical"] = None
                errors += 1
                entry["error"] = "unparseable output: " + ("hs" if hk is None else "rs")
            else:
                compared += 1
                same = hk == rk
                entry["identical"] = same
                if same:
                    identical += 1
                else:
                    hc, rc = collections.Counter(hk), collections.Counter(rk)
                    only_hs = sorted((hc - rc).elements())
                    only_rs = sorted((rc - hc).elements())
                    entry["only_hs"] = [fmt_key(k) for k in only_hs]
                    entry["only_rs"] = [fmt_key(k) for k in only_rs]
                    code_only_hs.update(k[2] for k in only_hs)
                    code_only_rs.update(k[2] for k in only_rs)
                    mismatches.append({"file": f, "only_hs": entry["only_hs"], "only_rs": entry["only_rs"]})
            per_file.append(entry)

        rs_rss = [v for v in rs["rss"].values() if v]
        hs_rss = [v for v in hs["rss"].values() if v]
        summary = {
            "name": name, "url": repo["url"], "commit": repo["commit"],
            "files": len(files), "lines": sum(lines.values()),
            "hs_wall_s": hs["wall_total"], "rs_wall_s": rs["wall_total"],
            "speedup": hs["wall_total"] / rs["wall_total"] if rs["wall_total"] else None,
            "hs_peak_rss_kb": max(hs_rss) if hs_rss else None,
            "rs_peak_rss_kb": max(rs_rss) if rs_rss else None,
            "hs_status_counts": dict(collections.Counter(hs["status"].values())),
            "rs_status_counts": dict(collections.Counter(rs["status"].values())),
            "files_compared": compared, "files_identical": identical,
            "files_identical_pct": 100.0 * identical / compared if compared else None,
            "files_unparseable": errors,
            "mismatches": mismatches,
            "per_file": per_file,
        }
        results.append(summary)
        print(f"[{name}] hs {summary['hs_wall_s']:.2f}s rs {summary['rs_wall_s']:.2f}s "
              f"identical {identical}/{compared}", file=sys.stderr, flush=True)

    machine["loadavg_end"] = os.getloadavg()[0]
    totals = {
        "files": sum(r["files"] for r in results),
        "lines": sum(r["lines"] for r in results),
        "files_compared": sum(r["files_compared"] for r in results),
        "files_identical": sum(r["files_identical"] for r in results),
        "hs_wall_s": sum(r["hs_wall_s"] for r in results),
        "rs_wall_s": sum(r["rs_wall_s"] for r in results),
    }
    totals["files_identical_pct"] = (100.0 * totals["files_identical"] / totals["files_compared"]
                                     if totals["files_compared"] else None)
    totals["speedup"] = totals["hs_wall_s"] / totals["rs_wall_s"] if totals["rs_wall_s"] else None
    config = {"runs": RUNS, "timeout_s": TIMEOUT, "mem_kb": MEM_KB, "args": ARGS}
    out = {"machine": machine, "config": config, "totals": totals, "repos": results,
           "disagreeing_codes": {
               "only_hs": dict(code_only_hs.most_common()),
               "only_rs": dict(code_only_rs.most_common())}}
    # the per-file list is kept in the JSON only; the markdown lists mismatches
    with open(os.path.join(RESULTS, "real-repos.json"), "w") as f:
        json.dump(out, f, indent=1)
        f.write("\n")
    write_markdown(out, os.path.join(RESULTS, "real-repos.md"))
    print("wrote", os.path.join(RESULTS, "real-repos.json"), file=sys.stderr)


def mb(kb):
    return "-" if kb is None else f"{kb / 1024:.1f}"


def write_markdown(out, path):
    m, t = out["machine"], out["totals"]
    L = []
    L.append("# Real-repo benchmark: Haskell ShellCheck vs shelllint")
    L.append("")
    L.append(f"Generated {m['date_utc']} by `bench/run_real.sh`. Raw data: `bench/results/real-repos.json`.")
    L.append("")
    L.append("## Machine")
    L.append("")
    L.append(f"- CPU: {m['cpu']} ({m['logical_cpus']} logical CPUs), RAM {m['mem_total_gb']} GB")
    L.append(f"- Kernel {m['kernel']}, {m['os']}")
    L.append(f"- Haskell: `{m['hs_version'][0] if m['hs_version'] else '?'}` (`koalaman/shellcheck:latest`, static binary)")
    L.append(f"- Rust: `{m['rs_version']}`, release build (LTO, codegen-units=1), {m['rs_binary_bytes']} bytes")
    L.append(f"- {m['rustc']}, Python {m['python']}")
    L.append(f"- Load average (1 min) at start {m['loadavg_start']:.2f}, at end {m['loadavg_end']:.2f}."
             " Other agents share this machine, so timings carry noise.")
    L.append(f"- Caps: {m['memory_cap']}. {m['method']}.")
    L.append(f"- Flags: `{' '.join(out['config']['args'])}` (json1 on stdout, shell from shebang)")
    L.append("")
    L.append("## Summary")
    L.append("")
    L.append(f"- {t['files']} files, {t['lines']} lines across {len(out['repos'])} repos")
    L.append(f"- Total wall time (sum of per-repo medians): Haskell {t['hs_wall_s']:.2f} s, "
             f"Rust {t['rs_wall_s']:.2f} s, speedup {t['speedup']:.1f}x")
    if t["files_identical_pct"] is not None:
        L.append(f"- Diagnostics identical on {t['files_identical']} of {t['files_compared']} "
                 f"comparable files ({t['files_identical_pct']:.1f}%), compared on (line, column, code)")
    L.append("")
    L.append("## Per repo")
    L.append("")
    L.append("| repo | files | lines | Haskell time (s) | Rust time (s) | speedup | Haskell peak RSS (MB) | Rust peak RSS (MB) | identical files |")
    L.append("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
    for r in out["repos"]:
        sp = f"{r['speedup']:.1f}x" if r["speedup"] else "-"
        ident = (f"{r['files_identical']}/{r['files_compared']} ({r['files_identical_pct']:.1f}%)"
                 if r["files_compared"] else "-")
        L.append(f"| {r['name']} | {r['files']} | {r['lines']} | {r['hs_wall_s']:.2f} | {r['rs_wall_s']:.2f} | "
                 f"{sp} | {mb(r['hs_peak_rss_kb'])} | {mb(r['rs_peak_rss_kb'])} | {ident} |")
    L.append("")
    L.append("Run statuses that were not `ok` (timeouts, crashes, exit codes other than 0/1):")
    L.append("")
    bad = []
    for r in out["repos"]:
        for side in ("hs", "rs"):
            c = {k: v for k, v in r[f"{side}_status_counts"].items() if k != "ok"}
            if c:
                bad.append(f"- {r['name']} {side}: {c}")
    L.extend(bad or ["- none"])
    L.append("")
    L.append("## Disagreements")
    L.append("")
    if out["disagreeing_codes"]["only_hs"] or out["disagreeing_codes"]["only_rs"]:
        L.append("| SC code | reported only by Haskell | reported only by Rust |")
        L.append("|---|---:|---:|")
        codes = sorted(set(out["disagreeing_codes"]["only_hs"]) | set(out["disagreeing_codes"]["only_rs"]))
        for c in codes:
            L.append(f"| SC{c} | {out['disagreeing_codes']['only_hs'].get(c, 0)} | "
                     f"{out['disagreeing_codes']['only_rs'].get(c, 0)} |")
    else:
        L.append("No diagnostic differences.")
    L.append("")
    L.append("Files with any difference (`only_hs` = Haskell only, `only_rs` = Rust only):")
    L.append("")
    any_mm = False
    for r in out["repos"]:
        for mm in r["mismatches"]:
            any_mm = True
            L.append(f"- `{r['name']}/{mm['file']}`")
            if mm["only_hs"]:
                L.append(f"  - only_hs: {', '.join(mm['only_hs'])}")
            if mm["only_rs"]:
                L.append(f"  - only_rs: {', '.join(mm['only_rs'])}")
    if not any_mm:
        L.append("- none")
    L.append("")
    with open(path, "w") as f:
        f.write("\n".join(L))


if __name__ == "__main__":
    main()
