#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Generate crates/mcp/data/codes.json, the static catalogue behind the MCP
server's `explain` and `list_checks` tools.

Usage: python3 -I tools/gen_mcp_codes.py <repo-root>

For every SC code the ported checks emit (crates/checks/src), it records:
  code      the number (2086)
  check     the Haskell check name that emits it (from corpus/checks.json)
  severity  error | warning | info | style (from the c.err/c.warn/... call)
  message   the first message string literal used for it
  bad       shortest script whose real ShellCheck output includes the code
            (corpus/tests.jsonl, oracle_codes)
  good      shortest script the same check stays silent on (expect_fires false)
Optional checks' descriptions are not stored here: the server reads them from
the analyzer at run time.
"""
import json
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
corpus = root / "corpus"
checks_src = root / "crates" / "checks" / "src"
out = root / "crates" / "mcp" / "data" / "codes.json"

CALL = re.compile(
    r'\.(err|warn|info|style)(?:_with_fix)?\(\s*[^,;]{1,200}?,\s*(\d{4})\s*,\s*"((?:[^"\\]|\\.)*)"'
)
SEV = {"err": "error", "warn": "warning", "info": "info", "style": "style"}

found = {}
for path in sorted(checks_src.rglob("*.rs")):
    for m in CALL.finditer(path.read_text(encoding="utf-8")):
        sev, code, msg = SEV[m.group(1)], int(m.group(2)), m.group(3)
        # Undo the Rust string escaping of the literal.
        msg = msg.replace('\\"', '"').replace("\\\\", "\\")
        found.setdefault(code, (sev, msg))

owner = {}
for rec in json.load(open(corpus / "checks.json")).values():
    for code in rec.get("codes", []) + rec.get("codes_direct", []):
        owner.setdefault(code, rec["name"])

tests = [json.loads(l) for l in open(corpus / "tests.jsonl")]
tests = [t for t in tests if isinstance(t.get("script"), str)]

def shortest(recs):
    return min(recs, key=lambda t: len(t["script"]))["script"] if recs else None

entries = []
for code in sorted(found):
    sev, msg = found[code]
    check = owner.get(code)
    bad = shortest([t for t in tests if code in t.get("oracle_codes", []) and t.get("oracle_agrees", True)])
    good = shortest([
        t for t in tests
        if check and t.get("target") == check and t.get("expect_fires") is False
        and code not in t.get("oracle_codes", [])
    ])
    entries.append({"code": code, "check": check, "severity": sev, "message": msg, "bad": bad, "good": good})

out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(entries, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
print(f"{len(entries)} codes; with bad example: {sum(e['bad'] is not None for e in entries)}; "
      f"with good example: {sum(e['good'] is not None for e in entries)}; "
      f"with check name: {sum(e['check'] is not None for e in entries)}")
