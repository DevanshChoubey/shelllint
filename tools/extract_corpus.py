#!/usr/bin/env python3
"""Extract ShellCheck's inline Haskell `prop_` tests into a JSON corpus.

Usage: python3 -I tools/extract_corpus.py <shellcheck-src-dir> <out-dir>

Writes:
  <out-dir>/tests.jsonl   one record per prop_ test
  <out-dir>/checks.json   one record per top-level check function:
                          module, line span, SC codes it emits (direct and via
                          same-module helpers), and its test names
"""
import json
import re
import sys
from pathlib import Path

MODULES = [
    "Analytics.hs",
    "Checks/Commands.hs",
    "Checks/ShellSupport.hs",
    "Checks/ControlFlow.hs",
    "Checks/Custom.hs",
    "Parser.hs",
    "ASTLib.hs",
    "AnalyzerLib.hs",
    "CFG.hs",
    "CFGAnalysis.hs",
    "Checker.hs",
    "Fixer.hs",
    "Formatter/Format.hs",
]

# helper -> (kind, expectation)
CORPUS_HELPERS = {
    "verify": ("node", True),
    "verifyNot": ("node", False),
    "verifyTree": ("tree", True),
    "verifyNotTree": ("tree", False),
    "verifyCodes": ("codes", None),
    "isOk": ("parse", "ok"),
    "isWarning": ("parse", "warning"),
    "isNotOk": ("parse", "error"),
}

SIMPLE_ESCAPES = {
    "n": "\n", "t": "\t", "r": "\r", "\\": "\\", '"': '"', "'": "'",
    "a": "\a", "b": "\b", "f": "\f", "v": "\v", "0": "\0",
}
ASCII_NAMES = {
    "NUL": 0, "SOH": 1, "STX": 2, "ETX": 3, "EOT": 4, "ENQ": 5, "ACK": 6,
    "BEL": 7, "BS": 8, "HT": 9, "LF": 10, "VT": 11, "FF": 12, "CR": 13,
    "SO": 14, "SI": 15, "DLE": 16, "DC1": 17, "DC2": 18, "DC3": 19,
    "DC4": 20, "NAK": 21, "SYN": 22, "ETB": 23, "CAN": 24, "EM": 25,
    "SUB": 26, "ESC": 27, "FS": 28, "GS": 29, "RS": 30, "US": 31, "SP": 32,
    "DEL": 127,
}


class ParseError(Exception):
    pass


def read_string(src, i):
    """Parse a Haskell string literal starting at src[i] == '"'.
    Returns (value, index_after_closing_quote)."""
    assert src[i] == '"'
    i += 1
    out = []
    while True:
        if i >= len(src):
            raise ParseError("unterminated string")
        c = src[i]
        if c == '"':
            return "".join(out), i + 1
        if c == "\n":
            raise ParseError("newline in string")
        if c != "\\":
            out.append(c)
            i += 1
            continue
        i += 1
        c = src[i]
        if c.isspace():  # string gap: \   ...   \
            while src[i].isspace():
                i += 1
            if src[i] != "\\":
                raise ParseError("bad string gap")
            i += 1
            continue
        if c == "&":
            i += 1
            continue
        if c.isdigit():
            m = re.match(r"\d+", src[i:])
            out.append(chr(int(m.group(0))))
            i += len(m.group(0))
            continue
        if c == "x":
            m = re.match(r"[0-9a-fA-F]+", src[i + 1:])
            out.append(chr(int(m.group(0), 16)))
            i += 1 + len(m.group(0))
            continue
        if c == "o":
            m = re.match(r"[0-7]+", src[i + 1:])
            out.append(chr(int(m.group(0), 8)))
            i += 1 + len(m.group(0))
            continue
        if c == "^":
            out.append(chr(ord(src[i + 1]) - 64))
            i += 2
            continue
        for name in sorted(ASCII_NAMES, key=len, reverse=True):
            if src.startswith(name, i):
                out.append(chr(ASCII_NAMES[name]))
                i += len(name)
                break
        else:
            if c in SIMPLE_ESCAPES:
                out.append(SIMPLE_ESCAPES[c])
                i += 1
            else:
                raise ParseError(f"unknown escape \\{c}")


def skip_ws(src, i):
    while i < len(src) and src[i].isspace():
        i += 1
    return i


def read_list_of_strings(src, i):
    """Parse [ "a", "b", ... ] starting at '['."""
    assert src[i] == "["
    i += 1
    items = []
    while True:
        i = skip_ws(src, i)
        if src[i] == "]":
            return items, i + 1
        if src[i] == ",":
            i += 1
            continue
        if src[i] == '"':
            s, i = read_string(src, i)
            items.append(s)
            continue
        raise ParseError(f"unexpected {src[i]!r} in list")


def read_string_expr(src, i):
    """Parse a string expression: literal, (expr), unlines [..], $ unlines [..],
    and `++` concatenations of those. Returns (value, index)."""
    i = skip_ws(src, i)
    if src.startswith("$", i):
        i = skip_ws(src, i + 1)
    parts = []
    while True:
        i = skip_ws(src, i)
        if src[i] == '"':
            s, i = read_string(src, i)
        elif src[i] == "(":
            s, i = read_string_expr(src, i + 1)
            i = skip_ws(src, i)
            if src[i] != ")":
                raise ParseError("expected )")
            i += 1
        elif src.startswith("unlines", i):
            i = skip_ws(src, i + len("unlines"))
            if src[i] == "$":
                i = skip_ws(src, i + 1)
            items, i = read_list_of_strings(src, i)
            s = "".join(x + "\n" for x in items)
        elif src.startswith("concat", i):
            i = skip_ws(src, i + len("concat"))
            items, i = read_list_of_strings(src, i)
            s = "".join(items)
        else:
            raise ParseError(f"unsupported string expression at {src[i:i+30]!r}")
        parts.append(s)
        j = skip_ws(src, i)
        if src.startswith("++", j):
            i = j + 2
            continue
        return "".join(parts), i


def split_top_level(text):
    """Yield (start_line, block_text) for each top-level definition block."""
    lines = text.split("\n")
    blocks = []
    cur_start, cur = None, []
    for n, line in enumerate(lines, 1):
        top = line and not line[0].isspace() and not line.startswith("--")
        if top:
            if cur:
                blocks.append((cur_start, "\n".join(cur)))
            cur_start, cur = n, [line]
        elif cur is not None and cur_start is not None:
            cur.append(line)
    if cur:
        blocks.append((cur_start, "\n".join(cur)))
    return blocks


CODE_RE = re.compile(
    r"\b(?:warn|err|info|style|warnWithFix|errWithFix|infoWithFix|styleWithFix|"
    r"makeComment|makeCommentWithFix|parseProblem\w*|parseNote\w*|"
    r"addComment|commentFor|unsupported|warnAt|errAt)\b[^\n]*?\b([1-3]\d{3})\b"
)
ANY_CODE_RE = re.compile(r"(?<![\w.])([1-3]\d{3})(?![\w.])")
DEF_NAME_RE = re.compile(r"^([a-z][A-Za-z0-9_']*)")


def strip_comments(block):
    block = re.sub(r"\{-.*?-\}", "", block, flags=re.S)
    return "\n".join(l.split("--", 1)[0] if not l.lstrip().startswith('"') else l
                     for l in block.split("\n"))


def main():
    src_dir, out_dir = Path(sys.argv[1]), Path(sys.argv[2])
    out_dir.mkdir(parents=True, exist_ok=True)
    tests, checks, failures = [], {}, []

    for mod in MODULES:
        path = src_dir / mod
        if not path.exists():
            continue
        text = path.read_text()
        blocks = split_top_level(text)
        defs = {}  # name -> (line, end_line, body)
        for start, block in blocks:
            m = DEF_NAME_RE.match(block)
            if not m:
                continue
            name = m.group(1)
            end = start + block.count("\n")
            if name.startswith("prop_"):
                body = block.split("=", 1)[1] if "=" in block else ""
                rec = {"module": mod, "name": name, "line": start,
                       "source": block.rstrip()}
                hm = re.match(r"\s*([A-Za-z]+)\b", body)
                helper = hm.group(1) if hm else None
                if helper in CORPUS_HELPERS:
                    kind, expect = CORPUS_HELPERS[helper]
                    rest = body[hm.end():]
                    try:
                        cm = re.match(r"\s*(\(?[A-Za-z][\w'.]*(?:\s+[A-Za-z][\w'.]*)*\)?|\([^)]*\))", rest)
                        target = cm.group(1).strip()
                        rest = rest[cm.end():]
                        codes = None
                        if kind == "codes":
                            lm = re.match(r"\s*\[([\d,\s]*)\]", rest)
                            codes = [int(c) for c in re.findall(r"\d+", lm.group(1))]
                            rest = rest[lm.end():]
                        script, _ = read_string_expr(rest, 0)
                        rec.update(kind=kind, helper=helper, target=target,
                                   script=script)
                        if kind == "codes":
                            rec["expect_codes"] = codes
                        elif kind == "parse":
                            rec["expect_parse"] = expect
                        else:
                            rec["expect_fires"] = expect
                    except (ParseError, AttributeError, IndexError) as e:
                        rec.update(kind="unparsed", helper=helper, error=str(e))
                        failures.append(rec)
                else:
                    rec.update(kind="unit", helper=helper)
                tests.append(rec)
            else:
                prev = defs.get(name)
                if prev:  # multiple equations / type signature + body
                    defs[name] = (prev[0], end, prev[2] + "\n" + block)
                else:
                    defs[name] = (start, end, block)

        # codes per definition, then transitive through same-module helpers
        direct = {}
        for name, (_, _, body) in defs.items():
            clean = strip_comments(body)
            direct[name] = set(int(c) for c in CODE_RE.findall(clean)) | (
                set(int(c) for c in ANY_CODE_RE.findall(clean))
                if mod.startswith("Checks/") or mod == "Analytics.hs" else set())
        refs = {}
        names = set(defs)
        for name, (_, _, body) in defs.items():
            words = set(re.findall(r"\b[a-z][A-Za-z0-9_']*\b", strip_comments(body)))
            refs[name] = (words & names) - {name}

        def closure(name, seen):
            out = set(direct.get(name, ()))
            for r in refs.get(name, ()):
                if r not in seen:
                    seen.add(r)
                    out |= closure(r, seen)
            return out

        for name, (line, end, body) in defs.items():
            if not re.match(r"check[A-Z]", name) and mod != "Parser.hs":
                continue
            if mod == "Parser.hs" and not re.match(r"read[A-Z]", name):
                continue
            checks[f"{mod}:{name}"] = {
                "module": mod, "name": name, "line": line, "end_line": end,
                "lines": end - line + 1,
                "codes_direct": sorted(direct[name]),
                "codes": sorted(closure(name, {name})),
                "helpers": sorted(refs[name]),
                "uses_cfg": bool(re.search(r"\bcfg|CFG|getIncomingState|getOutgoingState|dataflow", body)),
                "tests": [],
            }

    by_target = {}
    for t in tests:
        if t.get("target"):
            by_target.setdefault((t["module"], t["target"]), []).append(t["name"])
    for key, c in checks.items():
        c["tests"] = by_target.get((c["module"], c["name"]), [])

    with open(out_dir / "tests.jsonl", "w") as f:
        for t in tests:
            f.write(json.dumps(t, ensure_ascii=False) + "\n")
    with open(out_dir / "checks.json", "w") as f:
        json.dump(checks, f, indent=1, ensure_ascii=False)

    from collections import Counter
    print("tests by kind:", dict(Counter(t["kind"] for t in tests)))
    print("tests by module:", dict(Counter(t["module"] for t in tests)))
    print("check-like definitions:", len(checks))
    print("unparsed:", len(failures))
    for r in failures[:15]:
        print("  ", r["module"], r["name"], "-", r["error"])


if __name__ == "__main__":
    main()
