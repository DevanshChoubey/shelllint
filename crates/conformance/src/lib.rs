// SPDX-License-Identifier: GPL-3.0-only
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
