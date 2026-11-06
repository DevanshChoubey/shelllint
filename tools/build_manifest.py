#!/usr/bin/env python3
"""Group ShellCheck's checks into parallel porting tasks.

Usage: python3 -I tools/build_manifest.py <shellcheck-src-dir> <corpus-dir> <out.json>

Each task is a small, independent unit an agent can port in one sitting: one
or a few registered check functions from one module, their prop_ tests, the
SC codes they emit, and whether they need the control-flow (CFG) layer.
Codes the static scan missed are inferred from the oracle: codes present in
every expect-fires test of a check and absent from its expect-silent tests.
"""
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

src, corpus, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
checks = json.load(open(corpus / "checks.json"))
tests = [json.loads(l) for l in open(corpus / "tests.jsonl")]

LISTS = [
    ("Analytics.hs", "nodeChecks", "node"),
    ("Analytics.hs", "treeChecks", "tree"),
    ("Analytics.hs", "optionalChecks", "optional"),
    ("Analytics.hs", "optionalTreeChecks", "optional-tree"),
    ("Checks/Commands.hs", "commandChecks", "command"),
    ("Checks/Commands.hs", "optionalCommandChecks", "optional-command"),
    ("Checks/ShellSupport.hs", "checks", "shell-support"),
]
NAME_RE = r"\b(check[A-Z]\w*|subshellAssignmentCheck)\b"
# Haskell list constants that `map checkX list` registrations range over.
data_text = (src / "Data.hs").read_text()


def string_list(var):
    m = re.search(rf"^{var}\s*=\s*\[(.*?)\]", data_text, re.S | re.M)
    return re.findall(r'"([^"]*)"', m.group(1))


def registrations_in(body, name, reg, idx):
    """Every way `name` is registered in this list body, as the expression
    a CheckEntry's `name` must hold (what tests' check_expr() produces):
    plain `checkX`, `map checkX ("a" : list)` -> one `checkX "arg"` per
    argument, or `checkX' True`."""
    out = []
    for m in re.finditer(rf"\bmap\s+{name}\s+(?:\(\s*((?:\"\w+\"\s*:\s*)*)(\w+)\s*\)|(\w+))", body):
        lead = re.findall(r'"(\w+)"', m.group(1) or "")
        args = lead + string_list(m.group(2) or m.group(3))
        out += [{"registry": reg, "order": idx, "expr": f'{name} "{a}"'} for a in args]
    for m in re.finditer(rf"\b{name}'\s+(True|False)\b", body):
        out.append({"registry": reg, "order": idx, "expr": f"{name}' {m.group(1)}"})
    if not out:
        out.append({"registry": reg, "order": idx, "expr": name})
    if reg.startswith("optional"):
        for r in out:
            pos = body.find(r["expr"].split(" ")[0])
            names = re.findall(r'cdName\s*=\s*"([^"]*)"', body[:pos])
            r["optional_name"] = names[-1] if names else None
    return out


registered = {}  # (module, name) -> registry
order = {}  # (module, name) -> index in its Haskell registration list
registrations = defaultdict(list)  # (module, name) -> every registration
for mod, lst, reg in LISTS:
    text = (src / mod).read_text()
    m = re.search(rf"^{lst}\s*=\s*(.*?)^\S", text, re.S | re.M)
    if not m:
        continue
    for idx, name in enumerate(dict.fromkeys(re.findall(NAME_RE, m.group(1)))):
        if f"{mod}:{name}" in checks:
            if (mod, name) not in registered:
                registered[(mod, name)] = reg
                order[(mod, name)] = idx
            registrations[(mod, name)] += registrations_in(m.group(1), name, reg, idx)

test_by_name = {(t["module"], t["name"]): t for t in tests}


def inferred_codes(key):
    names = checks[f"{key[0]}:{key[1]}"]["tests"]
    ts = [t for t in (test_by_name[(key[0], n)] for n in names) if "oracle_codes" in t and "expect_fires" in t]
    pos = [set(t["oracle_codes"]) for t in ts if t["expect_fires"]]
    neg = set().union(*[set(t["oracle_codes"]) for t in ts if not t["expect_fires"]]) if ts else set()
    if not pos:
        return []
    common = set.intersection(*pos) - neg - {2148}
    return sorted(common)


units = []
for (mod, name), reg in registered.items():
    c = checks[f"{mod}:{name}"]
    codes = c["codes"] or inferred_codes((mod, name))
    units.append({
        "module": mod, "check": name, "registry": reg, "order": order[(mod, name)],
        "source_lines": [c["line"], c["end_line"]], "lines": c["lines"],
        "codes": codes, "codes_inferred": not c["codes"] and bool(codes),
        "uses_cfg": c["uses_cfg"] or name.endswith("Cfg"),
        "registrations": registrations[(mod, name)],
        "tests": c["tests"], "n_tests": len(c["tests"]),
    })

# Bundle: big or CFG checks alone; small checks packed in source order up to a budget.
units.sort(key=lambda u: (u["module"], u["source_lines"][0]))
BUDGET = 20  # Haskell lines per task (source incl. where-clauses)
tasks, cur = [], []


def flush():
    global cur
    if cur:
        tasks.append(cur)
        cur = []


for u in units:
    solo = u["uses_cfg"] or u["lines"] >= BUDGET
    if solo:
        flush()
        tasks.append([u])
        continue
    if cur and (cur[0]["module"] != u["module"] or sum(x["lines"] for x in cur) + u["lines"] > BUDGET):
        flush()
    cur.append(u)
flush()

manifest = []
for i, group in enumerate(tasks, 1):
    manifest.append({
        "id": f"T{i:03d}",
        "module": group[0]["module"],
        "phase": "cfg" if any(u["uses_cfg"] for u in group) else "checks",
        "checks": group,
        "haskell_lines": sum(u["lines"] for u in group),
        "n_tests": sum(u["n_tests"] for u in group),
        "codes": sorted({c for u in group for c in u["codes"]}),
    })

json.dump({"source_commit": "e11f964", "tasks": manifest}, open(out, "w"), indent=1)
print("registered checks:", len(units))
print("tasks:", len(manifest), "| cfg tasks:", sum(t["phase"] == "cfg" for t in manifest))
print("checks with no known codes:", [u["check"] for u in units if not u["codes"]])
print("tests covered:", sum(t["n_tests"] for t in manifest))
sizes = sorted(t["haskell_lines"] for t in manifest)
print("lines per task: min", sizes[0], "median", sizes[len(sizes)//2], "max", sizes[-1])
