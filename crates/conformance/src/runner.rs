// SPDX-License-Identifier: GPL-3.0-or-later
// Conformance harness for the port of ShellCheck (e11f964); no Haskell counterpart

use crate::{Outcome, Record, classify, parse_expectation};
use std::collections::BTreeMap;

/// Normalises a code list to a sorted set.
fn code_set(v: &[i64]) -> Vec<i64> {
    let mut v = v.to_vec();
    v.sort_unstable();
    v.dedup();
    v
}

/// Runs an analyzer check hook. `None` if the hook is absent or the check
/// is not registered.
#[allow(unused_variables)]
fn analyzer_codes(rec: &Record, kind: &str, script: &str) -> Option<Vec<i64>> {
    #[cfg(feature = "analyzer")]
    {
        return shellcheck_analyzer::testing::run_check(
            shellcheck_checks::CHECKS,
            &rec.module,
            &rec.check_expr()?,
            kind,
            script,
        );
    }
    #[allow(unreachable_code)]
    None
}

/// `Some(Some(r))` = parser ran; `Some(None)` is folded into `r`; outer
/// `None` = not ported (hook absent or parser name unknown).
#[allow(unused_variables)]
fn parser_result(target: &str, script: &str) -> Option<Option<bool>> {
    #[cfg(feature = "parser")]
    {
        let p = shellcheck_parser::testing::named_parser(target)?;
        return Some(shellcheck_parser::testing::parses_cleanly(p, script));
    }
    #[allow(unreachable_code)]
    None
}

/// Runs one record against the hooks.
pub fn run_record(rec: &Record) -> Outcome {
    classify(|| run_record_inner(rec))
}

fn run_record_inner(rec: &Record) -> Outcome {
    let (Some(script), Some(target)) = (rec.script.as_deref(), rec.target.as_deref()) else {
        return Outcome::Unported;
    };
    match rec.kind.as_str() {
        "node" | "tree" => {
            let Some(codes) = analyzer_codes(rec, &rec.kind, script) else {
                return Outcome::Unported;
            };
            let want = rec.expect_fires.unwrap_or(true);
            let fired = !codes.is_empty();
            if fired == want {
                Outcome::Pass
            } else {
                Outcome::Fail(format!(
                    "{target}: expected {}, got codes {codes:?}",
                    if want { "fires" } else { "silent" }
                ))
            }
        }
        "codes" => {
            let Some(codes) = analyzer_codes(rec, "codes", script) else {
                return Outcome::Unported;
            };
            let want = code_set(rec.expect_codes.as_deref().unwrap_or(&[]));
            let got = code_set(&codes);
            if got == want { Outcome::Pass } else { Outcome::Fail(format!("{target}: expected {want:?}, got {got:?}")) }
        }
        "parse" => {
            let Some(r) = parser_result(target, script) else {
                return Outcome::Unported;
            };
            let got = parse_expectation(r);
            let want = rec.expect_parse.as_deref().unwrap_or("ok");
            if got == want { Outcome::Pass } else { Outcome::Fail(format!("{target}: expected {want}, got {got}")) }
        }
        _ => Outcome::Unported,
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Counts {
    pub pass: usize,
    pub fail: usize,
    pub blocked: usize,
    pub unported: usize,
    /// `unit` / `unparsed` records; not run, not part of the four counts.
    pub skipped: usize,
}

impl Counts {
    pub fn add(&mut self, o: &Outcome) {
        match o {
            Outcome::Pass => self.pass += 1,
            Outcome::Fail(_) => self.fail += 1,
            Outcome::Blocked(_) => self.blocked += 1,
            Outcome::Unported => self.unported += 1,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Filter {
    pub module: Option<String>,
    pub kind: Option<String>,
    pub target: Option<String>,
}

impl Filter {
    pub fn matches(&self, r: &Record) -> bool {
        let m = self.module.as_ref().is_none_or(|m| r.module == *m || r.module.strip_suffix(".hs") == Some(m.as_str()));
        let k = self.kind.as_ref().is_none_or(|k| r.kind == *k);
        let t = self.target.as_ref().is_none_or(|t| r.target.as_deref() == Some(t));
        m && k && t
    }
}

/// Runs a filtered set of records with panic output silenced.
pub fn run_all<'a>(
    recs: impl IntoIterator<Item = &'a Record>,
    filter: &Filter,
) -> (Counts, Vec<(&'a Record, Outcome)>) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut counts = Counts::default();
    let mut results = Vec::new();
    for r in recs.into_iter().filter(|r| filter.matches(r)) {
        if !r.is_runnable() {
            counts.skipped += 1;
            continue;
        }
        let o = run_record(r);
        counts.add(&o);
        results.push((r, o));
    }
    std::panic::set_hook(prev);
    (counts, results)
}

/// Full pipeline for parity: codes reported by the checker for a script.
#[allow(unused_variables)]
fn pipeline_codes(rec: &Record) -> Option<Vec<i64>> {
    #[cfg(feature = "checker")]
    {
        use shellcheck_core::interface::Shell;
        let script = rec.script.as_deref()?;
        let shell = match rec.oracle_shell.as_deref() {
            Some("sh") => Some(Shell::Sh),
            Some("bash") => Some(Shell::Bash),
            _ => None,
        };
        return classify_codes(|| shellcheck_checker::check_script_codes(script, shell));
    }
    #[allow(unreachable_code)]
    None
}

#[allow(dead_code)]
fn classify_codes<F: FnOnce() -> Vec<i64>>(f: F) -> Option<Vec<i64>> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok()
}

#[derive(Debug, Default)]
pub struct ParityResult {
    pub total: usize,
    pub matched: usize,
    pub available: bool,
    /// Keys of tests whose code set equals the oracle's.
    pub matches: BTreeMap<String, bool>,
    /// (key, ours, oracle) for every script whose code set differs.
    pub mismatches: Vec<(String, Vec<i64>, Vec<i64>)>,
}

pub fn run_parity(recs: &[Record]) -> ParityResult {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut res = ParityResult::default();
    for r in recs {
        let (Some(_), Some(oracle)) = (r.script.as_ref(), r.oracle_codes.as_ref()) else {
            continue;
        };
        if !r.is_runnable() {
            continue;
        }
        res.total += 1;
        if let Some(got) = pipeline_codes(r) {
            res.available = true;
            if code_set(&got) == code_set(oracle) {
                res.matched += 1;
                res.matches.insert(r.key(), true);
            } else {
                let mut g: Vec<i64> = code_set(&got).into_iter().collect();
                let mut o: Vec<i64> = code_set(oracle).into_iter().collect();
                g.sort();
                o.sort();
                res.mismatches.push((r.key(), g, o));
            }
        }
    }
    std::panic::set_hook(prev);
    res
}

// ---- full-fidelity parity ---------------------------------------------------

use serde_json::Value;

/// How a script's comment list differs from the oracle's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiffKind {
    /// The code is reported a different number of times (or only on one side).
    Count,
    /// Same code and occurrence, different line/column/endLine/endColumn.
    Position,
    Message,
    Severity,
    Fix,
    /// Same comments, different output order.
    Order,
}

impl DiffKind {
    pub fn name(self) -> &'static str {
        match self {
            DiffKind::Count => "count",
            DiffKind::Position => "position",
            DiffKind::Message => "message",
            DiffKind::Severity => "severity",
            DiffKind::Fix => "fix",
            DiffKind::Order => "order",
        }
    }
}

/// Drops the `file` field: ours is checked as `script.sh`, the oracle's
/// files were `tNNNNN.sh`.
fn normalise_comment(c: &Value) -> Value {
    let mut c = c.clone();
    if let Value::Object(m) = &mut c {
        m.remove("file");
    }
    c
}

fn field<'a>(c: &'a Value, k: &str) -> &'a Value {
    c.get(k).unwrap_or(&Value::Null)
}

fn code_of(c: &Value) -> i64 {
    field(c, "code").as_i64().unwrap_or(0)
}

/// One comment as a single line, for diffs.
pub fn render_comment(c: &Value) -> String {
    let n = |k| field(c, k).as_i64().unwrap_or(0);
    let mut s = format!(
        "{}:{}-{}:{} {} SC{} {}",
        n("line"),
        n("column"),
        n("endLine"),
        n("endColumn"),
        field(c, "level").as_str().unwrap_or("?"),
        code_of(c),
        field(c, "message").as_str().unwrap_or("?"),
    );
    let fix = field(c, "fix");
    if !fix.is_null() {
        s.push_str(&format!(" fix={fix}"));
    }
    s
}

/// Classifies the differences between two (normalised) comment lists as
/// (code, kind) pairs. Occurrences of a code are paired in output order.
/// Empty iff the lists are equal.
pub fn classify_diff(ours: &[Value], oracle: &[Value]) -> Vec<(i64, DiffKind)> {
    use std::collections::BTreeSet;
    if ours == oracle {
        return vec![];
    }
    let mut out = BTreeSet::new();
    let codes: BTreeSet<i64> = ours.iter().chain(oracle).map(code_of).collect();
    for code in codes {
        let a: Vec<&Value> = ours.iter().filter(|c| code_of(c) == code).collect();
        let b: Vec<&Value> = oracle.iter().filter(|c| code_of(c) == code).collect();
        if a.len() != b.len() {
            out.insert((code, DiffKind::Count));
            continue;
        }
        for (x, y) in a.iter().zip(&b) {
            if ["line", "column", "endLine", "endColumn"].iter().any(|k| field(x, k) != field(y, k)) {
                out.insert((code, DiffKind::Position));
            }
            if field(x, "message") != field(y, "message") {
                out.insert((code, DiffKind::Message));
            }
            if field(x, "level") != field(y, "level") {
                out.insert((code, DiffKind::Severity));
            }
            if field(x, "fix") != field(y, "fix") {
                out.insert((code, DiffKind::Fix));
            }
        }
    }
    if out.is_empty() {
        // Only the order differs: blame the first oracle comment out of place.
        let i = ours.iter().zip(oracle).position(|(x, y)| x != y).unwrap_or(0);
        out.insert((oracle.get(i).map(code_of).unwrap_or(0), DiffKind::Order));
    }
    out.into_iter().collect()
}

/// Line diff (LCS) of two comment lists: `  ` same, `- ` oracle only,
/// `+ ` ours only.
pub fn diff_lines(ours: &[Value], oracle: &[Value]) -> Vec<String> {
    let a: Vec<String> = oracle.iter().map(render_comment).collect();
    let b: Vec<String> = ours.iter().map(render_comment).collect();
    let (n, m) = (a.len(), b.len());
    let mut t = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if a[i] == b[j] { t[i + 1][j + 1] + 1 } else { t[i + 1][j].max(t[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, vec![]);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            out.push(format!("  {}", a[i]));
            i += 1;
            j += 1;
        } else if j < m && (i == n || t[i][j + 1] >= t[i + 1][j]) {
            out.push(format!("+ {}", b[j]));
            j += 1;
        } else {
            out.push(format!("- {}", a[i]));
            i += 1;
        }
    }
    out
}

/// The CLI's `ioInterface` as the oracle ran it for the corpus: no `-x`, no
/// source paths, `--norc`, no .editorconfig, and only `tNNNNN.sh` files in
/// the directory. Sourced files are therefore never readable, and the error
/// text is the CLI's (the mock's "File not included in mock." would show up
/// as an SC1091 message difference that the real tool never has).
#[cfg(feature = "checker")]
struct OracleRunInterface;

#[cfg(feature = "checker")]
impl shellcheck_core::interface::SystemInterface for OracleRunInterface {
    fn read_file(&self, external: Option<bool>, file: &str) -> Result<String, String> {
        Err(match external {
            Some(true) => format!("{file}: openBinaryFile: does not exist (No such file or directory)"),
            Some(false) => {
                format!("{file} was not specified as input, and external files were disabled via directive.")
            }
            None => format!("{file} was not specified as input (see shellcheck -x)."),
        })
    }
}

/// What the formatter reads back for the checked file: the corpus script
/// itself, as the CLI's formatter would re-read `script.sh` from disk.
#[cfg(feature = "checker")]
struct ScriptContents<'a>(&'a str);

#[cfg(feature = "checker")]
impl shellcheck_core::interface::SystemInterface for ScriptContents<'_> {
    fn read_file(&self, _external: Option<bool>, file: &str) -> Result<String, String> {
        if file == "script.sh" { Ok(self.0.to_string()) } else { OracleRunInterface.read_file(None, file) }
    }
}

/// The json1 comments our pipeline produces for a corpus script, run as the
/// oracle was (`-f json1 --norc -o all [-s shell]`), through our Json1
/// formatter, with `file` removed.
#[allow(unused_variables)]
pub fn pipeline_json1(rec: &Record) -> Option<Vec<Value>> {
    #[cfg(feature = "checker")]
    {
        use shellcheck_core::interface::{CheckSpec, Shell, empty_check_spec};
        use shellcheck_formatter::Formatter;
        let script = rec.script.as_deref()?;
        let shell = match rec.oracle_shell.as_deref() {
            Some("sh") => Some(Shell::Sh),
            Some("bash") => Some(Shell::Bash),
            _ => None,
        };
        let run = || {
            let spec = CheckSpec {
                filename: "script.sh".to_string(),
                script: script.to_string(),
                ignore_rc: true,
                shell_type_override: shell,
                optional_checks: vec!["all".to_string()],
                ..empty_check_spec()
            };
            let result = shellcheck_checker::check_script(&OracleRunInterface, &spec);
            let mut f = shellcheck_formatter::json1::format();
            let mut buf: Vec<u8> = vec![];
            f.on_result(&result, &ScriptContents(script), &mut buf);
            f.footer(&mut buf);
            buf
        };
        let buf = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)).ok()?;
        let v: Value = serde_json::from_slice(&buf).ok()?;
        return Some(v.get("comments")?.as_array()?.iter().map(normalise_comment).collect());
    }
    #[allow(unreachable_code)]
    None
}

#[derive(Debug, Clone)]
pub struct FullMismatch {
    pub key: String,
    pub kinds: Vec<(i64, DiffKind)>,
    pub ours: Vec<Value>,
    pub oracle: Vec<Value>,
}

#[derive(Debug, Default)]
pub struct FullParityResult {
    pub total: usize,
    pub matched: usize,
    pub available: bool,
    /// Scripts the oracle file has no usable entry for.
    pub missing_oracle: Vec<String>,
    /// Scripts our pipeline panicked on.
    pub crashed: Vec<String>,
    pub matches: BTreeMap<String, bool>,
    pub mismatches: Vec<FullMismatch>,
}

/// Compares the exact ordered json1 comment list (minus `file`) of every
/// script that has oracle codes against corpus/oracle-full.jsonl.
pub fn run_parity_full(recs: &[Record], oracle: &BTreeMap<String, crate::OracleFull>) -> FullParityResult {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut res = FullParityResult { available: cfg!(feature = "checker"), ..Default::default() };
    for r in recs {
        if r.script.is_none() || r.oracle_codes.is_none() || !r.is_runnable() {
            continue;
        }
        res.total += 1;
        let key = r.key();
        let Some(want) = oracle.get(&key).and_then(|o| o.comments.as_ref()) else {
            res.missing_oracle.push(key);
            continue;
        };
        if !res.available {
            continue;
        }
        let want: Vec<Value> = want.iter().map(normalise_comment).collect();
        let Some(got) = pipeline_json1(r) else {
            res.crashed.push(key);
            continue;
        };
        let kinds = classify_diff(&got, &want);
        if kinds.is_empty() {
            res.matched += 1;
            res.matches.insert(key, true);
        } else {
            res.mismatches.push(FullMismatch { key, kinds, ours: got, oracle: want });
        }
    }
    std::panic::set_hook(prev);
    res
}

/// (code, kind) -> keys of the mismatching scripts.
pub fn group_mismatches(m: &[FullMismatch]) -> BTreeMap<(i64, DiffKind), Vec<String>> {
    let mut g: BTreeMap<(i64, DiffKind), Vec<String>> = BTreeMap::new();
    for x in m {
        for k in &x.kinds {
            g.entry(*k).or_default().push(x.key.clone());
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(kind: &str) -> Record {
        Record {
            module: "Analytics.hs".into(),
            name: "prop_x".into(),
            kind: kind.into(),
            helper: None,
            target: Some("checkX".into()),
            source: None,
            script: Some("echo".into()),
            expect_fires: Some(true),
            expect_codes: None,
            expect_parse: Some("ok".into()),
            oracle_codes: None,
            oracle_shell: None,
        }
    }

    #[test]
    fn hooks_off_means_unported() {
        if cfg!(not(any(feature = "parser", feature = "analyzer"))) {
            for k in ["node", "tree", "codes", "parse"] {
                assert_eq!(run_record(&rec(k)), Outcome::Unported);
            }
        }
    }

    fn c(line: i64, col: i64, code: i64, msg: &str) -> Value {
        serde_json::json!({"line": line, "endLine": line, "column": col, "endColumn": col + 1,
            "level": "info", "code": code, "message": msg, "fix": null})
    }

    #[test]
    fn classify_diff_kinds() {
        let a = vec![c(1, 1, 2086, "m"), c(2, 1, 2249, "d")];
        assert!(classify_diff(&a, &a).is_empty());
        let moved = vec![c(1, 1, 2086, "m"), c(2, 6, 2249, "d")];
        assert_eq!(classify_diff(&moved, &a), vec![(2249, DiffKind::Position)]);
        let msg = vec![c(1, 1, 2086, "x"), c(2, 1, 2249, "d")];
        assert_eq!(classify_diff(&msg, &a), vec![(2086, DiffKind::Message)]);
        let mut sev = a.clone();
        sev[0]["level"] = "style".into();
        assert_eq!(classify_diff(&sev, &a), vec![(2086, DiffKind::Severity)]);
        let mut fix = a.clone();
        fix[1]["fix"] = serde_json::json!({"replacements": []});
        assert_eq!(classify_diff(&fix, &a), vec![(2249, DiffKind::Fix)]);
        let extra = vec![c(1, 1, 2086, "m"), c(1, 1, 2086, "m"), c(2, 1, 2249, "d")];
        assert_eq!(classify_diff(&extra, &a), vec![(2086, DiffKind::Count)]);
        let swapped = vec![a[1].clone(), a[0].clone()];
        assert_eq!(classify_diff(&swapped, &a), vec![(2086, DiffKind::Order)]);
    }

    #[test]
    fn diff_lines_marks_sides() {
        let a = vec![c(1, 1, 2086, "m")];
        let b = vec![c(1, 1, 2086, "m"), c(2, 1, 2249, "d")];
        let d = diff_lines(&b, &a);
        assert_eq!(d.len(), 2);
        assert!(d[0].starts_with("  1:1-1:2 info SC2086"));
        assert!(d[1].starts_with("+ 2:1-2:2 info SC2249"));
    }

    /// The SC2249 case-expression range bug the code-set gate missed is
    /// visible to the full comparison (or, once fixed, the script matches).
    #[cfg(feature = "checker")]
    #[test]
    fn full_pipeline_matches_json1_shape() {
        let mut r = rec("node");
        r.script = Some("#!/bin/bash\necho $1\n".into());
        let got = pipeline_json1(&r).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(
            render_comment(&got[0]),
            "2:6-2:8 info SC2086 Double quote to prevent globbing and word splitting. fix={\"replacements\":[{\"column\":6,\"endColumn\":6,\"endLine\":2,\"insertionPoint\":\"afterEnd\",\"line\":2,\"precedence\":7,\"replacement\":\"\\\"\"},{\"column\":8,\"endColumn\":8,\"endLine\":2,\"insertionPoint\":\"beforeStart\",\"line\":2,\"precedence\":7,\"replacement\":\"\\\"\"}]}"
        );
        assert!(got[0].get("file").is_none());
    }

    #[test]
    fn filter_accepts_module_without_suffix() {
        let f = Filter { module: Some("Analytics".into()), ..Default::default() };
        assert!(f.matches(&rec("node")));
    }
}
