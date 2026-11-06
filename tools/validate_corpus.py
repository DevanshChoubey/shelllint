#!/usr/bin/env python3
"""Run every corpus test script through the reference ShellCheck (Docker) and
record what it reports, then measure how well each test's expectation agrees.

Usage: python3 -I tools/validate_corpus.py <corpus-dir> [docker-image]

Adds `oracle_codes` to each node/tree/codes/parse test in tests.jsonl and
writes <corpus-dir>/validation.json with agreement stats per check.

A unit-level test (`verify checkX`) runs one check in isolation, while the
oracle runs everything, so agreement is judged on the SC codes the check is
known to emit (from checks.json). Disagreement is expected for a minority of
tests (shell-specific options, codes shared between checks, optional checks);
those are listed so porting agents treat them as unit-only.
"""
import json
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

corpus = Path(sys.argv[1])
image = sys.argv[2] if len(sys.argv) > 2 else "koalaman/shellcheck:latest"
tests = [json.loads(l) for l in open(corpus / "tests.jsonl")]
checks = json.load(open(corpus / "checks.json"))
code_of = {}
for key, c in checks.items():
    code_of[(c["module"], c["name"])] = set(c["codes"])

runnable = [t for t in tests if t["kind"] in ("node", "tree", "codes", "parse")]

with tempfile.TemporaryDirectory() as d:
    d = Path(d)
    groups = defaultdict(list)  # needs -s bash or not
    for i, t in enumerate(runnable):
        name = f"t{i:05d}.sh"
        (d / name).write_text(t["script"])
        if t["script"].startswith("#!"):
            shell = None
        elif t["module"] == "Checks/ShellSupport.hs" and "Bashisms" in t.get("target", ""):
            shell = "sh"  # bashism tests exercise a POSIX-sh dialect
        else:
            shell = "bash"
        t["oracle_shell"] = shell
        groups[shell].append((name, t))
    results = defaultdict(list)
    for shell, items in groups.items():
        for start in range(0, len(items), 400):
            chunk = items[start:start + 400]
            args = ["docker", "run", "--rm", "-v", f"{d}:/w:ro", "-w", "/w", image,
                    "-f", "json1", "--norc", "-o", "all"]
            if shell:
                args += ["-s", shell]
            args += [n for n, _ in chunk]
            out = subprocess.run(args, capture_output=True, text=True)
            try:
                data = json.loads(out.stdout or "{}")
            except json.JSONDecodeError:
                print("bad oracle output:", out.stderr[:500])
                sys.exit(1)
            for c in data.get("comments", []):
                results[c["file"]].append(c["code"])
    for name, t in [x for v in groups.values() for x in v]:
        t["oracle_codes"] = sorted(set(results.get(name, [])))

stats = defaultdict(lambda: {"agree": 0, "disagree": 0, "unknown_codes": 0, "disagreeing_tests": []})
totals = defaultdict(int)
for t in runnable:
    got = set(t["oracle_codes"])
    if t["kind"] == "parse":
        totals["parse"] += 1
        continue
    key = (t["module"], t["target"])
    s = stats[f"{key[0]}:{key[1]}"]
    if t["kind"] == "codes":
        ok = set(t["expect_codes"]) <= got if t["expect_codes"] else True
    else:
        want = code_of.get(key, set())
        if not want:
            s["unknown_codes"] += 1
            totals["unknown_codes"] += 1
            continue
        fired = bool(got & want)
        ok = fired == t["expect_fires"]
    t["oracle_agrees"] = ok
    s["agree" if ok else "disagree"] += 1
    totals["agree" if ok else "disagree"] += 1
    if not ok:
        s["disagreeing_tests"].append(t["name"])

with open(corpus / "tests.jsonl", "w") as f:
    for t in tests:
        f.write(json.dumps(t, ensure_ascii=False) + "\n")
json.dump({"totals": dict(totals), "per_check": stats}, open(corpus / "validation.json", "w"), indent=1)
print(dict(totals))
worst = sorted(stats.items(), key=lambda kv: -kv[1]["disagree"])[:12]
for k, v in worst:
    print(f"  {k}: agree={v['agree']} disagree={v['disagree']} unknown={v['unknown_codes']}")
