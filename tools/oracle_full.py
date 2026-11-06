#!/usr/bin/env python3
"""Record the oracle's full json1 output for every runnable corpus script.

Usage: python3 -I tools/oracle_full.py <corpus-dir> <shellcheck-binary>

For each record in tests.jsonl that has `oracle_codes`, runs the reference
ShellCheck the same way tools/validate_corpus.py did (`-f json1 --norc -o all`
plus `-s <oracle_shell>` when the record has one) and writes one line per
script to <corpus-dir>/oracle-full.jsonl:

    {"key": "<module>:<name>", "shell": <oracle_shell>, "comments": [...]}

`comments` is the oracle's json1 comment list for that file, in output order,
each with file, line, column, endLine, endColumn, level, code, message and
fix. tests.jsonl itself is not modified.

Every oracle process runs under `ulimit -v 4000000` and `timeout 60`. Scripts
are batched per shell; a batch that fails is retried one file at a time, and a
file that still fails is recorded with `"error"` instead of `comments`.
"""
import json
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

BATCH = 200

corpus = Path(sys.argv[1])
binary = sys.argv[2]
tests = [json.loads(l) for l in open(corpus / "tests.jsonl")]
runnable = [t for t in tests
            if t["kind"] in ("node", "tree", "codes", "parse") and "oracle_codes" in t]


def run(d, shell, names):
    cmd = [binary, "-f", "json1", "--norc", "-o", "all"]
    if shell:
        cmd += ["-s", shell]
    cmd += names
    sh = "ulimit -v 4000000; exec timeout 60 \"$@\""
    out = subprocess.run(["bash", "-c", sh, "oracle", *cmd], cwd=d,
                         capture_output=True, text=True)
    # exit 0 = clean, 1 = comments; anything else is a failure
    if out.returncode not in (0, 1):
        return None
    try:
        return json.loads(out.stdout)["comments"]
    except (json.JSONDecodeError, KeyError):
        return None


with tempfile.TemporaryDirectory() as d:
    groups = defaultdict(list)
    for i, t in enumerate(runnable):
        name = f"t{i:05d}.sh"
        (Path(d) / name).write_text(t["script"])
        groups[t.get("oracle_shell")].append((name, t))
    results = {}
    errors = {}
    for shell, items in groups.items():
        for start in range(0, len(items), BATCH):
            chunk = [n for n, _ in items[start:start + BATCH]]
            got = run(d, shell, chunk)
            if got is None:
                for n in chunk:
                    one = run(d, shell, [n])
                    if one is None:
                        errors[n] = "oracle failed or timed out"
                    else:
                        results[n] = one
                continue
            by_file = defaultdict(list)
            for c in got:
                by_file[c["file"]].append(c)
            for n in chunk:
                results[n] = by_file.get(n, [])

keys = set()
with open(corpus / "oracle-full.jsonl", "w") as f:
    for i, t in enumerate(runnable):
        name = f"t{i:05d}.sh"
        key = f'{t["module"]}:{t["name"]}'
        assert key not in keys, key
        keys.add(key)
        rec = {"key": key, "shell": t.get("oracle_shell")}
        if name in errors:
            rec["error"] = errors[name]
        else:
            rec["comments"] = results[name]
        f.write(json.dumps(rec, ensure_ascii=False) + "\n")

drift = [f'{t["module"]}:{t["name"]}' for i, t in enumerate(runnable)
         if f"t{i:05d}.sh" in results
         and sorted({c["code"] for c in results[f"t{i:05d}.sh"]}) != t["oracle_codes"]]
print(f"{len(runnable)} scripts, {len(errors)} errors, "
      f"{len(drift)} whose code set differs from oracle_codes")
for k in drift:
    print("  drift:", k)
