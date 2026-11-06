// SPDX-License-Identifier: GPL-3.0-or-later
// Conformance harness for the port of ShellCheck (e11f964); no Haskell counterpart
//
// Loads corpus/tests.jsonl and runs each record against the hooks exposed by
// the parser / analyzer / checker crates (feature-gated until those exist).

pub mod runner;

use serde::Deserialize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

/// One line of corpus/tests.jsonl. Fields absent for a kind are `None`.
#[derive(Debug, Clone, Deserialize)]
pub struct Record {
    pub module: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub helper: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub expect_fires: Option<bool>,
    #[serde(default)]
    pub expect_codes: Option<Vec<i64>>,
    #[serde(default)]
    pub expect_parse: Option<String>,
    #[serde(default)]
    pub oracle_codes: Option<Vec<i64>>,
    #[serde(default)]
    pub oracle_shell: Option<String>,
}

impl Record {
    /// Stable identifier used in the parity baseline.
    pub fn key(&self) -> String {
        format!("{}:{}", self.module, self.name)
    }

    /// The check expression the test applies its helper to. `target` is
    /// cut at the first space, so for `verify (checkSudoArgs "sudo") ...` it
    /// is `(checkSudoArgs`; the full expression (`checkSudoArgs "sudo"`) is
    /// recovered from `source`.
    pub fn check_expr(&self) -> Option<String> {
        let t = self.target.as_deref()?;
        if !t.starts_with('(') {
            return Some(t.to_string());
        }
        let src = self.source.as_deref().unwrap_or("");
        if let Some(i) = src.find(t) {
            let (mut depth, mut in_str) = (0i32, false);
            for (j, c) in src[i..].char_indices() {
                match c {
                    '"' => in_str = !in_str,
                    '(' if !in_str => depth += 1,
                    ')' if !in_str => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(src[i + 1..i + j].trim().to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
        Some(t.trim_start_matches('(').trim_end_matches(')').to_string())
    }

    /// Kinds the harness executes; `unit` and `unparsed` are skipped.
    pub fn is_runnable(&self) -> bool {
        matches!(self.kind.as_str(), "node" | "tree" | "codes" | "parse")
    }
}

/// Default corpus directory (`<repo>/corpus`).
pub fn default_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

pub fn parse_corpus(text: &str) -> Result<Vec<Record>, serde_json::Error> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect()
}

pub fn load_corpus(path: &Path) -> Result<Vec<Record>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_corpus(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Loads the parity baseline. A missing file is an empty baseline; any other
/// read error or a parse failure is an `Err` (so the regression gate cannot
/// be silently disabled by a corrupt file).
pub fn load_baseline(path: &Path) -> Result<std::collections::BTreeMap<String, bool>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// One line of corpus/oracle-full.jsonl (written by tools/oracle_full.py):
/// the oracle's complete json1 comment list for one runnable script.
#[derive(Debug, Clone, Deserialize)]
pub struct OracleFull {
    pub key: String,
    #[serde(default)]
    pub shell: Option<String>,
    /// json1 comments in output order. `None` when the oracle failed.
    #[serde(default)]
    pub comments: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Loads oracle-full.jsonl keyed by `Record::key`. A duplicate key is an
/// error, since the comparison could otherwise pick the wrong script.
pub fn load_oracle_full(
    path: &Path,
) -> Result<std::collections::BTreeMap<String, OracleFull>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut map = std::collections::BTreeMap::new();
    for (i, l) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let r: OracleFull = serde_json::from_str(l)
            .map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))?;
        if map.contains_key(&r.key) {
            return Err(format!("{}:{}: duplicate key {}", path.display(), i + 1, r.key));
        }
        map.insert(r.key.clone(), r);
    }
    Ok(map)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail(String),
    /// Panicked with a message starting `unported:` (the `unported!` macro).
    Blocked(String),
    /// The target is not registered / the hook feature is disabled.
    Unported,
}

/// Maps `parses_cleanly` output to the corpus `expect_parse` vocabulary.
pub fn parse_expectation(r: Option<bool>) -> &'static str {
    match r {
        Some(true) => "ok",
        Some(false) => "warning",
        None => "error",
    }
}

fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic>".to_string()
    }
}

/// Runs `f`, catching panics. A panic starting `unported:` is `Blocked`,
/// any other panic is `Fail`.
pub fn classify<F: FnOnce() -> Outcome>(f: F) -> Outcome {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(o) => o,
        Err(p) => {
            let msg = panic_message(&*p);
            if msg.starts_with("unported:") {
                Outcome::Blocked(msg)
            } else {
                Outcome::Fail(format!("panic: {msg}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_corpus() {
        let recs = load_corpus(&default_corpus_dir().join("tests.jsonl")).unwrap();
        // The brief said 2,105; that is the number of records with oracle
        // data. The file actually has 2,334 records (incl. 225 unit, 4 unparsed).
        assert_eq!(recs.len(), 2334);
        assert_eq!(recs.iter().filter(|r| r.kind == "node").count(), 1386);
        assert_eq!(recs.iter().filter(|r| r.oracle_codes.is_some()).count(), 2105);
    }

    #[test]
    fn oracle_full_covers_every_parity_script() {
        let recs = load_corpus(&default_corpus_dir().join("tests.jsonl")).unwrap();
        let full = load_oracle_full(&default_corpus_dir().join("oracle-full.jsonl")).unwrap();
        assert_eq!(full.len(), 2105);
        for r in recs.iter().filter(|r| r.oracle_codes.is_some()) {
            let o = full.get(&r.key()).unwrap_or_else(|| panic!("no oracle-full entry for {}", r.key()));
            assert_eq!(o.shell, r.oracle_shell, "{}", r.key());
            let comments = o.comments.as_ref().unwrap();
            // the full record agrees with the code-set record it extends
            let mut codes: Vec<i64> = comments.iter().map(|c| c["code"].as_i64().unwrap()).collect();
            codes.sort_unstable();
            codes.dedup();
            assert_eq!(&codes, r.oracle_codes.as_ref().unwrap(), "{}", r.key());
        }
        let dir = std::env::temp_dir().join(format!("sc-oracle-full-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dup = dir.join("dup.jsonl");
        std::fs::write(&dup, "{\"key\":\"a\",\"comments\":[]}\n{\"key\":\"a\",\"comments\":[]}\n").unwrap();
        assert!(load_oracle_full(&dup).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn baseline_loading() {
        let dir = std::env::temp_dir().join(format!("sc-baseline-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(load_baseline(&dir.join("missing.json")).unwrap().is_empty());
        let ok = dir.join("ok.json");
        std::fs::write(&ok, r#"{"A.hs:prop_a": true}"#).unwrap();
        assert_eq!(load_baseline(&ok).unwrap().len(), 1);
        let bad = dir.join("bad.json");
        std::fs::write(&bad, "<<<<<<< HEAD\n{}").unwrap();
        assert!(load_baseline(&bad).is_err());
        // a directory is a read error, not "missing"
        assert!(load_baseline(&dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn check_expr_recovers_arguments() {
        let mut r: Record = serde_json::from_str(
            r#"{"module":"Checks/Commands.hs","name":"prop_checkSudoArgs1","kind":"node",
                "target":"(checkSudoArgs","source":"prop_checkSudoArgs1 = verify (checkSudoArgs \"sudo\") \"sudo cd /root\""}"#,
        )
        .unwrap();
        assert_eq!(r.check_expr().as_deref(), Some("checkSudoArgs \"sudo\""));
        r.target = Some("checkTr".into());
        assert_eq!(r.check_expr().as_deref(), Some("checkTr"));
        let recs = load_corpus(&default_corpus_dir().join("tests.jsonl")).unwrap();
        let e = recs.iter().find(|r| r.name == "prop_checkUnassignedReferences38").unwrap();
        assert_eq!(e.check_expr().as_deref(), Some("checkUnassignedReferences' True"));
    }

    #[test]
    fn classifies_unported_panic() {
        let o = classify(|| panic!("unported: readFoo"));
        assert!(matches!(o, Outcome::Blocked(ref m) if m == "unported: readFoo"));
        let o = classify(|| panic!("boom"));
        assert!(matches!(o, Outcome::Fail(_)));
        assert_eq!(classify(|| Outcome::Pass), Outcome::Pass);
    }

    #[test]
    fn parse_expectation_mapping() {
        assert_eq!(parse_expectation(Some(true)), "ok");
        assert_eq!(parse_expectation(Some(false)), "warning");
        assert_eq!(parse_expectation(None), "error");
    }
}
