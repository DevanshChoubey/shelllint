#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build site/data.json for the dashboard from the bench and conformance outputs.

Inputs (all optional except the corpus, which always exists in the repo):
  bench/results/conformance.json   from bench/run_conformance.sh
  bench/results/real-repos.json    from bench/run_real.sh
  corpus/tests.jsonl, corpus/checks.json

Usage: python3 -I site/generate.py
"""
import datetime, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
RESULTS = os.path.join(ROOT, "bench", "results")
OUT = os.path.join(HERE, "data.json")
TEST_KINDS = {"node", "tree", "codes"}  # tests that name SC codes


def load_json(path):
    if not os.path.exists(path):
        return None, f"missing: {os.path.relpath(path, ROOT)}"
    with open(path) as f:
        return json.load(f), None


def conformance_section(notes):
    data, err = load_json(os.path.join(RESULTS, "conformance.json"))
    if data is None:
        notes.append(err + " (run bench/run_conformance.sh)")
        data = {"pass": None, "fail": None, "blocked": None, "unported": None,
                "skipped": None, "failures": []}
    total = sum(v for k, v in data.items() if k in ("pass", "fail", "blocked", "unported") and v is not None)
    data["total"] = total
    data["pass_pct"] = 100.0 * data["pass"] / total if data.get("pass") is not None and total else None
    return data


def corpus_section(failure_keys):
    """Per-SC-code coverage: how many corpus tests name each code."""
    recs = []
    with open(os.path.join(ROOT, "corpus", "tests.jsonl")) as f:
        for line in f:
            recs.append(json.loads(line))
    kinds = {}
    for r in recs:
        kinds[r["kind"]] = kinds.get(r["kind"], 0) + 1

    cover, failing = {}, {}
    for r in recs:
        if r["kind"] not in TEST_KINDS:
            continue
        codes = set(r.get("expect_codes") or []) | set(r.get("oracle_codes") or [])
        key = f'{r["module"]}:{r["name"]}'
        for c in codes:
            cover.setdefault(c, []).append(r["module"])
            if key in failure_keys:
                failing[c] = failing.get(c, 0) + 1

    registered = set()
    checks_path = os.path.join(ROOT, "corpus", "checks.json")
    checks = {}
    if os.path.exists(checks_path):
        with open(checks_path) as f:
            checks = json.load(f)
        for info in checks.values():
            registered.update(info.get("codes", []))

    all_codes = sorted(set(cover) | registered)
    rows = []
    for c in all_codes:
        mods = cover.get(c, [])
        rows.append({
            "code": c,
            "tests": len(mods),
            "failing": failing.get(c, 0),
            "modules": sorted(set(mods)),
            "registered": c in registered,
        })
    return {
        "records": len(recs),
        "by_kind": kinds,
        "codes_with_tests": sum(1 for r in rows if r["tests"]),
        "codes_registered": len(registered),
        "codes_registered_untested": sum(1 for r in rows if r["registered"] and not r["tests"]),
        "per_code": rows,
    }


def real_section(notes):
    data, err = load_json(os.path.join(RESULTS, "real-repos.json"))
    if data is None:
        notes.append(err + " (run bench/run_real.sh)")
        return None
    repos = []
    for r in data["repos"]:
        repos.append({
            "name": r["name"], "url": r["url"], "commit": r["commit"],
            "files": r["files"], "lines": r["lines"],
            "hs_wall_s": r["hs_wall_s"], "rs_wall_s": r["rs_wall_s"], "speedup": r["speedup"],
            "hs_peak_rss_mb": r["hs_peak_rss_kb"] / 1024 if r["hs_peak_rss_kb"] else None,
            "rs_peak_rss_mb": r["rs_peak_rss_kb"] / 1024 if r["rs_peak_rss_kb"] else None,
            "files_compared": r["files_compared"], "files_identical": r["files_identical"],
            "identical_pct": r["files_identical_pct"],
            "unparseable": r["files_unparseable"],
            "mismatch_files": [
                {"file": m["file"], "only_hs": m["only_hs"], "only_rs": m["only_rs"]}
                for m in r["mismatches"]
            ],
        })
    return {
        "machine": data["machine"],
        "config": data["config"],
        "totals": data["totals"],
        "repos": repos,
        "disagreeing_codes": data["disagreeing_codes"],
    }


def main():
    notes = []
    conf = conformance_section(notes)
    failure_keys = {f["key"] for f in conf.get("failures", [])}
    real = real_section(notes)
    out = {
        "generated_at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "project": "shellcheck-rust",
        "reference": "ShellCheck v0.11.0-171-ge11f964 (Haskell, koalaman/shellcheck:latest)",
        "notes": notes,
        "conformance": {k: v for k, v in conf.items() if k != "failures"},
        "conformance_failures": conf.get("failures", []),
        "corpus": corpus_section(failure_keys),
        "real": real,
    }
    with open(OUT, "w") as f:
        json.dump(out, f, indent=1)
        f.write("\n")
    print(f"wrote {os.path.relpath(OUT, ROOT)}" + (f" (notes: {len(notes)})" if notes else ""), file=sys.stderr)


if __name__ == "__main__":
    main()
