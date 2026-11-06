// SPDX-License-Identifier: GPL-3.0-only
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
            if got == want {
                Outcome::Pass
            } else {
                Outcome::Fail(format!("{target}: expected {want:?}, got {got:?}"))
            }
        }
        "parse" => {
            let Some(r) = parser_result(target, script) else {
                return Outcome::Unported;
            };
            let got = parse_expectation(r);
            let want = rec.expect_parse.as_deref().unwrap_or("ok");
            if got == want {
                Outcome::Pass
            } else {
                Outcome::Fail(format!("{target}: expected {want}, got {got}"))
            }
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
        let m = self.module.as_ref().is_none_or(|m| {
            r.module == *m || r.module.strip_suffix(".hs") == Some(m.as_str())
        });
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

    #[test]
    fn filter_accepts_module_without_suffix() {
        let f = Filter { module: Some("Analytics".into()), ..Default::default() };
        assert!(f.matches(&rec("node")));
    }
}
