#!/usr/bin/env python3
"""Print the brief for one Phase 2 check-porting task.

Usage: python3 -I tools/task_prompt.py T042

Reads corpus/tasks.json (the manifest), corpus/checks.json and
corpus/tests.jsonl, and prints: the checks to port with their Haskell line
spans, registry/order and SC codes, the file to create for each, the CheckEntry
values to register, the same-module helpers they use (and where a Rust port
already exists), the prop_ tests, the gate commands, and the commit message.
The conventions are in docs/porting-a-check.md.
"""
import json
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GUIDE = "docs/porting-a-check.md"
HASKELL = "reference/shellcheck/src/ShellCheck"
MODULE_DIRS = {
    "Analytics.hs": "analytics",
    "Checks/Commands.hs": "commands",
    "Checks/ShellSupport.hs": "shell_support",
}
KINDS = {
    "node": "CheckKind::Node(f)",
    "tree": "CheckKind::Tree(f)",
    "optional-tree": "CheckKind::Tree(f), or CheckKind::Node(f) for `nodeChecksToTreeCheck [f]`",
    "command": "CheckKind::Command(CommandName::Exactly/Basename(\"cmd\"), f)",
    "optional-command": "CheckKind::Command(CommandName::Exactly/Basename(\"cmd\"), f)",
    "shell-support": "CheckKind::ForShell(&[Shell::...], f)",
}
# Where already-ported Rust helpers live, searched for `pub fn <snake_name>`.
RUST_HELPER_FILES = [
    ("crates/checks/src/support.rs", "shellcheck_checks::support / crate::support"),
    ("crates/core/src/astlib.rs", "shellcheck_core::astlib"),
    ("crates/analyzer/src/analyzerlib.rs", "shellcheck_analyzer::analyzerlib"),
    ("crates/core/src/prelude.rs", "shellcheck_core::prelude"),
    ("crates/core/src/regex.rs", "shellcheck_core::regex"),
]
RUNNABLE = {"node", "tree", "codes"}


def snake(name):
    """readNormalWord -> read_normal_word; checkPS1Assignments -> check_ps1_assignments."""
    s = name.replace("'", "_")
    s = re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", s)
    return s.lower().strip("_")


def rust_helpers():
    found = {}
    for path, where in RUST_HELPER_FILES:
        p = ROOT / path
        if p.exists():
            for m in re.finditer(r"^pub (?:fn |use \S+::)(\w+)", p.read_text(), re.M):
                found.setdefault(m.group(1), where)
    return found


def main():
    if len(sys.argv) != 2:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    task_id = sys.argv[1].upper()
    manifest = json.load(open(ROOT / "corpus/tasks.json"))
    checks = json.load(open(ROOT / "corpus/checks.json"))
    tests = {}
    for line in open(ROOT / "corpus/tests.jsonl"):
        t = json.loads(line)
        tests[(t["module"], t["name"])] = t
    task = next((t for t in manifest["tasks"] if t["id"] == task_id), None)
    if task is None:
        print(f"no task {task_id} in corpus/tasks.json", file=sys.stderr)
        return 1
    have = rust_helpers()
    names = [c["check"] for c in task["checks"]]
    out = []
    w = out.append

    w(f"# Task {task_id}: port {', '.join(names)} ({task['module']})")
    w("")
    w(f"You are porting {len(names)} ShellCheck check(s) to Rust, in the shellcheck-rust workspace.")
    w(f"Read {GUIDE} first; it is binding. Haskell source: {HASKELL}/{task['module']} (commit {manifest['source_commit']}).")
    w(f"Oracle (tiebreaker for behaviour): printf '%s\\n' 'SCRIPT' | docker run --rm -i koalaman/shellcheck:latest -s bash -f gcc -")
    if task["phase"] == "cfg":
        w("")
        w("PHASE cfg: this task needs the control-flow/data-flow layer (CFG.hs, CFGAnalysis.hs).")
        w("If `Params::cfg_analysis` carries no data yet, stop and report BLOCKED instead of guessing.")
    w("")
    w(f"Haskell lines in this task: {task['haskell_lines']}; tests: {task['n_tests']}; SC codes: {task['codes']}")

    gates = []
    for c in task["checks"]:
        mod, name = c["module"], c["check"]
        key = f"{mod}:{name}"
        info = checks.get(key, {})
        lo, hi = c["source_lines"]
        path = f"crates/checks/src/{MODULE_DIRS[mod]}/{snake(name)}.rs"
        w("")
        w(f"## {name}")
        w("")
        w(f"- Haskell: {HASKELL}/{mod} lines {lo}-{hi} ({c['lines']} lines, including `where` clauses); "
          f"its prop_ tests sit just above it.")
        w(f"- SC codes: {c['codes']}" + (" (inferred from the oracle; the source scan found none)" if c["codes_inferred"] else ""))
        if c["uses_cfg"]:
            w("- Uses the CFG / data-flow layer.")
        w(f"- File to create: {path}")
        w(f"  Header: `// SPDX-License-Identifier: GPL-3.0-only` then "
          f"`// Port of ShellCheck src/ShellCheck/{mod} `{name}`, lines {lo}-{hi} ({manifest['source_commit']})`")
        regs = c.get("registrations") or [{"registry": c["registry"], "order": c["order"], "expr": name}]
        w(f"- Register {'one CheckEntry' if len(regs) == 1 else f'{len(regs)} CheckEntry statics'} "
          f"(`#[linkme::distributed_slice(CHECKS)]`):")
        for r in regs:
            opt = ""
            if r["registry"].startswith("optional"):
                opt = (f", optional: Some(StaticCheckDescription {{ name: \"{r.get('optional_name')}\", ... }}) "
                       f"(copy cdId/cdDescription/cdPositive/cdNegative from the source)")
            w(f"    module: \"{mod}\", name: {json.dumps(r['expr'])}, registry: \"{r['registry']}\", "
              f"order: {r['order']}{opt}, kind: {KINDS[r['registry']]}")
        if len(regs) > 1:
            w("  Several registrations: one static per expression (ENTRY_SUDO, ENTRY_DOAS, ...), each with a small "
              "wrapper fn (NodeFn is a plain fn pointer, so it cannot capture the argument). "
              "The `name` must equal the expression exactly; see the guide, \"Argful checks\".")
        helpers = [h for h in info.get("helpers", []) if h != "return"]
        if helpers:
            w("- Same-module helpers it references (port locally unless listed as ported):")
            for h in helpers:
                where = have.get(snake(h))
                status = f"ported: {where}::{snake(h)}" if where else "not ported: put it in your file (private fn)"
                w(f"    {h}: {status}")
        ts = [tests[(mod, n)] for n in c["tests"] if (mod, n) in tests]
        runnable = [t for t in ts if t["kind"] in RUNNABLE]
        w(f"- Tests ({len(c['tests'])}): {', '.join(c['tests']) if c['tests'] else '(none)'}")
        if len(runnable) != len(ts):
            w(f"  ({len(ts) - len(runnable)} of them are `unit` tests the harness does not run; port them as Rust "
              f"#[test]s in crates/checks/tests/ only if they test this check's helpers.)")
        for target, n in Counter(t["target"] for t in runnable).items():
            gates.append((mod, target, n))

    w("")
    w("## Gate")
    w("")
    w("Each command below must print `pass N / fail 0 / blocked 0 / unported 0` with the N shown:")
    for mod, target, n in gates:
        w(f"    cargo run -q -p shellcheck-conformance -- report --module {mod} --target {shlex_quote(target)}   # pass {n}")
    w("Then `cargo test --workspace` with no failures and no warnings.")
    w("")
    w("## Rules (details in the guide)")
    w("")
    w("- Only create the files listed above. Never edit another check's file, support.rs, mod.rs, lib.rs, the analyzer, "
      "core or parser; if you need a change there, report NEEDS_CONTEXT with the exact change.")
    w("- Keep definitions in Haskell order; port literally; mark necessary differences with `// DIVERGENCE:`.")
    w("- No `unsafe`, no new dependencies.")
    w("")
    w("## Commit")
    w("")
    w(f"    checks: port {', '.join(names)} ({task_id})")
    w("")
    w("    Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>")
    print("\n".join(out))
    return 0


def shlex_quote(s):
    return s if re.fullmatch(r"[\w./-]+", s) else "'" + s.replace("'", "'\\''") + "'"


if __name__ == "__main__":
    sys.exit(main())
